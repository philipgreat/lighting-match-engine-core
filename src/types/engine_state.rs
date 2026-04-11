use crate::types::{
    AnyOrderBook, AuctionKind, AuctionSession, CallAuctionOrderSubmitError, CallAuctionPool,
    EngineState, MarketPhase, MarketStructureConfig, MatchOutcome, OrderBook,
    OrderFlags, OrderRequest, OrderSide, OrderSubmitError, PhaseTransitionError, PriceType,
    SubmitOrderError,
};

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

impl EngineState {
    pub fn new(instance_tag: [u8; 16], product_id: u16, order_book: AnyOrderBook) -> Self {
        Self::new_with_market_structure(
            instance_tag,
            product_id,
            order_book,
            MarketStructureConfig::default(),
        )
    }

    pub fn new_with_market_structure(
        instance_tag: [u8; 16],
        product_id: u16,
        order_book: AnyOrderBook,
        market_structure: MarketStructureConfig,
    ) -> Self {
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;

        EngineState {
            instance_tag,
            product_id,
            order_book,
            market_structure,
            phase: MarketPhase::PreOpen,
            active_auction: None,
            matched_orders: 0,
            total_received_orders: 0,
            start_time: now_nanos,
        }
    }

    pub fn increase_match(&mut self) {
        self.matched_orders += 1;
    }

    pub fn match_order(&mut self, new_order: OrderRequest) -> Result<(), OrderSubmitError> {
        self.total_received_orders += 1;
        self.order_book.match_order(new_order.clone()).map_err(|source| OrderSubmitError {
            order: new_order,
            source,
        })?;
        self.matched_orders += self.order_book.last_outcome().total_count() as u64;
        Ok(())
    }

    pub fn submit_order(&mut self, new_order: OrderRequest) -> Result<(), SubmitOrderError> {
        match self.phase {
            MarketPhase::AuctionOrderEntry(_) => self
                .queue_call_auction_order(new_order)
                .map_err(SubmitOrderError::CallAuction),
            MarketPhase::ContinuousTrading => self
                .match_order(new_order)
                .map_err(SubmitOrderError::Continuous),
            phase => Err(SubmitOrderError::InvalidPhase { phase }),
        }
    }

    pub fn transition_to(
        &mut self,
        next: MarketPhase,
        current_ts: u64,
    ) -> Result<(), PhaseTransitionError> {
        match (self.phase, next) {
            (MarketPhase::PreOpen, MarketPhase::AuctionOrderEntry(kind)) => {
                self.ensure_auction_kind_enabled(kind)?;
                self.start_auction_session(kind, current_ts)?;
            }
            (MarketPhase::PreOpen, MarketPhase::ContinuousTrading) => {}
            (MarketPhase::AuctionOrderEntry(current), MarketPhase::AuctionFrozen(next_kind))
                if current == next_kind =>
            {
                self.active_auction_mut(MarketPhase::AuctionOrderEntry(current))?
                    .frozen_at = Some(current_ts);
            }
            (MarketPhase::AuctionFrozen(current), MarketPhase::AuctionMatching(next_kind))
                if current == next_kind =>
            {}
            (MarketPhase::AuctionMatching(AuctionKind::Opening), MarketPhase::ContinuousTrading) => {
                self.active_auction = None;
            }
            (MarketPhase::AuctionMatching(AuctionKind::Closing), MarketPhase::Closed) => {
                self.active_auction = None;
            }
            (
                MarketPhase::AuctionMatching(AuctionKind::VolatilityInterruption),
                MarketPhase::ContinuousTrading,
            ) => {
                self.active_auction = None;
            }
            (MarketPhase::ContinuousTrading, MarketPhase::AuctionOrderEntry(kind)) => {
                self.ensure_auction_kind_enabled(kind)?;
                self.start_auction_session(kind, current_ts)?;
            }
            (MarketPhase::ContinuousTrading, MarketPhase::TradingHalt) => {}
            (MarketPhase::ContinuousTrading, MarketPhase::Closed) => {}
            (MarketPhase::TradingHalt, MarketPhase::ContinuousTrading) => {}
            _ if self.phase == next => return Ok(()),
            (from, to) => {
                return Err(PhaseTransitionError::InvalidTransition { from, to });
            }
        }

        self.phase = next;
        Ok(())
    }

    pub fn queue_call_auction_order(
        &mut self,
        new_order: OrderRequest,
    ) -> Result<(), CallAuctionOrderSubmitError> {
        self.total_received_orders += 1;
        self.active_auction_mut(self.phase)
            .expect("queue_call_auction_order requires an active auction session")
            .pool
            .add_order(new_order.clone())
            .map_err(|source| CallAuctionOrderSubmitError {
                order: new_order,
                source,
            })
    }

    pub fn execute_call_auction(
        &mut self,
        price_tick: u64,
        current_ts: u64,
    ) -> Result<MatchOutcome, PhaseTransitionError> {
        match self.phase {
            MarketPhase::AuctionMatching(kind) => {
                let instance_tag = self.instance_tag;
                let product_id = self.product_id;
                let outcome = {
                    let session = self.active_auction_mut(MarketPhase::AuctionMatching(kind))?;
                    let outcome = session.pool.execute_auction(
                        price_tick,
                        instance_tag,
                        product_id,
                        current_ts,
                    );
                    session.matched_at = Some(current_ts);
                    session.last_outcome = outcome.clone();
                    outcome
                };

                self.matched_orders += outcome.total_count() as u64;
                Ok(outcome)
            }
            phase => Err(PhaseTransitionError::InvalidPhaseForAuctionExecution { phase }),
        }
    }

    pub fn active_auction(&self) -> Option<&AuctionSession> {
        self.active_auction.as_ref()
    }

    fn ensure_auction_kind_enabled(
        &self,
        kind: AuctionKind,
    ) -> Result<(), PhaseTransitionError> {
        let enabled = match kind {
            AuctionKind::Opening => self.market_structure.has_opening_auction,
            AuctionKind::Closing => self.market_structure.has_closing_auction,
            AuctionKind::VolatilityInterruption => self.market_structure.allows_volatility_auction,
        };

        if enabled {
            Ok(())
        } else {
            Err(PhaseTransitionError::UnsupportedAuctionKind { kind })
        }
    }

    fn start_auction_session(
        &mut self,
        kind: AuctionKind,
        current_ts: u64,
    ) -> Result<(), PhaseTransitionError> {
        if let Some(active) = &self.active_auction {
            return Err(PhaseTransitionError::ActiveAuctionAlreadyExists { kind: active.kind });
        }

        self.active_auction = Some(AuctionSession {
            kind,
            pool: CallAuctionPool::new(1000),
            started_at: current_ts,
            frozen_at: None,
            matched_at: None,
            last_outcome: MatchOutcome::new(0),
        });
        Ok(())
    }

    fn active_auction_mut(
        &mut self,
        phase: MarketPhase,
    ) -> Result<&mut AuctionSession, PhaseTransitionError> {
        self.active_auction
            .as_mut()
            .ok_or(PhaseTransitionError::MissingActiveAuction { phase })
    }

    pub fn execute_active_auction(
        &mut self,
        price_tick: u64,
        current_ts: u64,
    ) -> Result<MatchOutcome, PhaseTransitionError> {
        self.execute_call_auction(price_tick, current_ts)
    }

    pub fn load_sample_test_book(&mut self, test_order_book_size: u32) -> Result<(), OrderSubmitError> {
        for i in 0..test_order_book_size {
            let order = self.create_buy_order(i);
            self.order_book.seed_order(order.clone()).map_err(|source| OrderSubmitError {
                order,
                source,
            })?;
        }
        for i in 0..test_order_book_size {
            let order = self.create_sell_order(i, test_order_book_size);
            self.order_book.seed_order(order.clone()).map_err(|source| OrderSubmitError {
                order,
                source,
            })?;
        }
        Ok(())
    }

    pub fn create_buy_order(&self, index: u32) -> OrderRequest {
        let time_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;
        OrderRequest {
            product_id: self.product_id,
            order_id: (index + 1) as u64,
            side: OrderSide::Buy,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            price: (index + 1) as u64,
            quantity: 2,
            submit_time: time_now,
            expire_time: time_now + 1000 * 1000 * 1000 * 1000 * 10,
            _padding: [0u8; 24],
        }
    }

    pub fn create_sell_order(&self, index: u32, size: u32) -> OrderRequest {
        let time_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;

        OrderRequest {
            product_id: self.product_id,
            order_id: (size + index + 1) as u64,
            side: OrderSide::Sell,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            price: (size + 1 + index) as u64,
            quantity: 2,
            submit_time: time_now,
            expire_time: time_now + 1000 * 1000 * 1000 * 1000 * 10,
            _padding: [0u8; 24],
        }
    }
}

pub struct StatusBroadcaster {
    state: Arc<EngineState>,
}

impl StatusBroadcaster {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        AnyOrderBook, CallAuctionPoolError, OrderBookError, PhaseTransitionError,
    };

    #[test]
    fn engine_state_wraps_order_book_error_with_order_context() {
        let order_book = AnyOrderBook::dense(10, 100, 4, 8);
        let mut state = EngineState::new([0; 16], 7, order_book);

        let order = OrderRequest {
            product_id: 7,
            order_id: 99,
            side: OrderSide::Buy,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            price: 105,
            quantity: 1,
            submit_time: 0,
            expire_time: 0,
            _padding: [0u8; 24],
        };

        let err = state.match_order(order.clone()).unwrap_err();
        assert_eq!(err.order.order_id, 99);
        assert_eq!(err.order.product_id, 7);
        assert_eq!(
            err.source,
            OrderBookError::PriceNotOnTick {
                price: 105,
                base_price: 100,
                tick: 10,
            }
        );
    }

    #[test]
    fn engine_state_wraps_call_auction_error_with_order_context() {
        let order_book = AnyOrderBook::dense(10, 100, 4, 8);
        let mut state = EngineState::new([0; 16], 7, order_book);
        state
            .transition_to(MarketPhase::AuctionOrderEntry(AuctionKind::Opening), 1)
            .unwrap();

        let order = OrderRequest {
            product_id: 7,
            order_id: 100,
            side: OrderSide::Buy,
            price_type: PriceType::Market,
            flags: OrderFlags::default(),
            price: 0,
            quantity: 1,
            submit_time: 0,
            expire_time: 0,
            _padding: [0u8; 24],
        };

        let err = state.queue_call_auction_order(order.clone()).unwrap_err();
        assert_eq!(err.order.order_id, 100);
        assert_eq!(err.order.product_id, 7);
        assert_eq!(
            err.source,
            CallAuctionPoolError::UnsupportedPriceType {
                price_type: PriceType::Market,
            }
        );
    }

    #[test]
    fn engine_state_can_queue_and_execute_call_auction() {
        let order_book = AnyOrderBook::dense(10, 100, 4, 8);
        let mut state = EngineState::new([0; 16], 7, order_book);
        state
            .transition_to(MarketPhase::AuctionOrderEntry(AuctionKind::Opening), 1)
            .unwrap();

        let buy = OrderRequest {
            product_id: 7,
            order_id: 1,
            side: OrderSide::Buy,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            price: 110,
            quantity: 3,
            submit_time: 1,
            expire_time: 0,
            _padding: [0u8; 24],
        };
        let sell = OrderRequest {
            product_id: 7,
            order_id: 2,
            side: OrderSide::Sell,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            price: 100,
            quantity: 3,
            submit_time: 2,
            expire_time: 0,
            _padding: [0u8; 24],
        };

        state.queue_call_auction_order(buy).unwrap();
        state.queue_call_auction_order(sell).unwrap();

        state
            .transition_to(MarketPhase::AuctionFrozen(AuctionKind::Opening), 2)
            .unwrap();
        state
            .transition_to(MarketPhase::AuctionMatching(AuctionKind::Opening), 3)
            .unwrap();

        let outcome = state.execute_call_auction(10, 123).unwrap();

        assert_eq!(outcome.total_count(), 1);
        assert_eq!(outcome.trades[0].buy_order_id, 1);
        assert_eq!(outcome.trades[0].sell_order_id, 2);
        assert_eq!(outcome.trades[0].price, 100);
        assert_eq!(outcome.trades[0].quantity, 3);
        assert_eq!(state.matched_orders, 1);
        let auction = state.active_auction().unwrap();
        assert_eq!(auction.last_outcome.total_count(), 1);
        assert!(auction.pool.bids.is_empty());
        assert!(auction.pool.asks.is_empty());
    }

    #[test]
    fn transition_to_rejects_unsupported_closing_auction_by_default() {
        let order_book = AnyOrderBook::dense(10, 100, 4, 8);
        let mut state = EngineState::new([0; 16], 7, order_book);

        let err = state
            .transition_to(MarketPhase::AuctionOrderEntry(AuctionKind::Closing), 1)
            .unwrap_err();

        assert_eq!(
            err,
            PhaseTransitionError::UnsupportedAuctionKind {
                kind: AuctionKind::Closing,
            }
        );
    }

    #[test]
    fn submit_order_routes_by_market_phase() {
        let order_book = AnyOrderBook::dense(10, 100, 8, 8);
        let mut state = EngineState::new_with_market_structure(
            [0; 16],
            7,
            order_book,
            MarketStructureConfig {
                has_opening_auction: true,
                has_closing_auction: false,
                allows_volatility_auction: false,
            },
        );

        let auction_order = OrderRequest {
            product_id: 7,
            order_id: 1,
            side: OrderSide::Buy,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            price: 110,
            quantity: 1,
            submit_time: 1,
            expire_time: 0,
            _padding: [0u8; 24],
        };
        let continuous_order = OrderRequest {
            product_id: 7,
            order_id: 2,
            side: OrderSide::Buy,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            price: 100,
            quantity: 1,
            submit_time: 2,
            expire_time: 0,
            _padding: [0u8; 24],
        };

        assert!(matches!(
            state.submit_order(auction_order.clone()),
            Err(SubmitOrderError::InvalidPhase {
                phase: MarketPhase::PreOpen
            })
        ));

        state
            .transition_to(MarketPhase::AuctionOrderEntry(AuctionKind::Opening), 10)
            .unwrap();
        state.submit_order(auction_order).unwrap();
        assert_eq!(state.active_auction().unwrap().pool.bids.len(), 1);

        state
            .transition_to(MarketPhase::AuctionFrozen(AuctionKind::Opening), 11)
            .unwrap();
        state
            .transition_to(MarketPhase::AuctionMatching(AuctionKind::Opening), 12)
            .unwrap();
        state.execute_active_auction(10, 13).unwrap();
        state
            .transition_to(MarketPhase::ContinuousTrading, 14)
            .unwrap();

        state.submit_order(continuous_order).unwrap();
        assert_eq!(state.total_received_orders, 2);
    }
}
