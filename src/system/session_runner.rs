use crate::types::{
    AuctionKind, EngineState, MarketPhase, MatchOutcome, OrderFlags, OrderRequest, OrderSide,
    PriceType, SubmitOrderError, TradingSessionError,
};

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub opening_auction: MatchOutcome,
    pub closing_auction: Option<MatchOutcome>,
}

fn make_limit_order(
    product_id: u16,
    order_id: u64,
    side: OrderSide,
    price: u64,
    quantity: u32,
    submit_time: u64,
) -> OrderRequest {
    OrderRequest {
        product_id,
        side,
        price,
        price_type: PriceType::Limit,
        flags: OrderFlags::default(),
        quantity,
        order_id,
        submit_time,
        expire_time: 0,
        _padding: [0u8; 24],
    }
}

fn map_submit_error(err: SubmitOrderError) -> TradingSessionError {
    TradingSessionError::Submit(err)
}

fn run_call_auction_session(
    engine_state: &mut EngineState,
    kind: AuctionKind,
    tick: u64,
    current_ts: u64,
    orders: impl IntoIterator<Item = OrderRequest>,
    next_phase: MarketPhase,
) -> Result<MatchOutcome, TradingSessionError> {
    engine_state
        .transition_to(MarketPhase::AuctionOrderEntry(kind), current_ts)
        .map_err(TradingSessionError::Transition)?;

    for order in orders {
        engine_state.submit_order(order).map_err(map_submit_error)?;
    }

    engine_state
        .transition_to(MarketPhase::AuctionFrozen(kind), current_ts + 10)
        .map_err(TradingSessionError::Transition)?;
    engine_state
        .transition_to(MarketPhase::AuctionMatching(kind), current_ts + 20)
        .map_err(TradingSessionError::Transition)?;

    engine_state
        .execute_active_auction(tick, current_ts + 30)
        .map_err(TradingSessionError::Transition)?;
    let outcome = engine_state
        .take_active_auction_outcome()
        .map_err(TradingSessionError::Transition)?;

    engine_state
        .transition_to(next_phase, current_ts + 40)
        .map_err(TradingSessionError::Transition)?;

    Ok(outcome)
}

pub fn run_opening_call_auction(
    engine_state: &mut EngineState,
    tick: u64,
    current_ts: u64,
) -> Result<MatchOutcome, TradingSessionError> {
    let opening_orders = [
        make_limit_order(engine_state.product_id, 900_000_001, OrderSide::Buy, 110, 5, current_ts),
        make_limit_order(
            engine_state.product_id,
            900_000_002,
            OrderSide::Buy,
            100,
            3,
            current_ts + 1,
        ),
        make_limit_order(
            engine_state.product_id,
            900_000_003,
            OrderSide::Sell,
            100,
            4,
            current_ts + 2,
        ),
        make_limit_order(
            engine_state.product_id,
            900_000_004,
            OrderSide::Sell,
            110,
            2,
            current_ts + 3,
        ),
    ];

    run_call_auction_session(
        engine_state,
        AuctionKind::Opening,
        tick,
        current_ts,
        opening_orders,
        MarketPhase::ContinuousTrading,
    )
}

pub fn run_closing_call_auction(
    engine_state: &mut EngineState,
    tick: u64,
    current_ts: u64,
) -> Result<Option<MatchOutcome>, TradingSessionError> {
    if !engine_state.market_structure.has_closing_auction {
        return Ok(None);
    }

    let closing_orders = [
        make_limit_order(engine_state.product_id, 990_000_001, OrderSide::Buy, 120, 4, current_ts),
        make_limit_order(
            engine_state.product_id,
            990_000_002,
            OrderSide::Buy,
            110,
            3,
            current_ts + 1,
        ),
        make_limit_order(
            engine_state.product_id,
            990_000_003,
            OrderSide::Sell,
            110,
            5,
            current_ts + 2,
        ),
        make_limit_order(
            engine_state.product_id,
            990_000_004,
            OrderSide::Sell,
            120,
            1,
            current_ts + 3,
        ),
    ];

    let outcome = run_call_auction_session(
        engine_state,
        AuctionKind::Closing,
        tick,
        current_ts,
        closing_orders,
        MarketPhase::Closed,
    )?;

    Ok(Some(outcome))
}

pub fn run_demo_session(
    engine_state: &mut EngineState,
    tick: u64,
    opening_ts: u64,
    closing_ts: u64,
) -> Result<SessionSummary, TradingSessionError> {
    let opening_auction = run_opening_call_auction(engine_state, tick, opening_ts)?;
    let closing_auction = run_closing_call_auction(engine_state, tick, closing_ts)?;

    Ok(SessionSummary {
        opening_auction,
        closing_auction,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AnyOrderBook, MarketStructureConfig};

    #[test]
    fn opening_call_auction_runner_leaves_engine_in_continuous_trading() {
        let order_book = AnyOrderBook::dense(10, 100, 8, 16);
        let mut state = EngineState::new_with_market_structure(
            [0; 16],
            7,
            order_book,
            MarketStructureConfig::default(),
        );

        let outcome = run_opening_call_auction(&mut state, 10, 1_000).unwrap();

        assert_eq!(state.phase, MarketPhase::ContinuousTrading);
        assert_eq!(outcome.total_count(), 2);
        assert_eq!(outcome.trades[0].price, 110);
        assert_eq!(outcome.trades[1].price, 110);
        assert!(state.active_auction().is_none());
    }

    #[test]
    fn closing_call_auction_is_skipped_when_market_structure_disables_it() {
        let order_book = AnyOrderBook::dense(10, 100, 8, 16);
        let mut state = EngineState::new_with_market_structure(
            [0; 16],
            7,
            order_book,
            MarketStructureConfig::default(),
        );

        run_opening_call_auction(&mut state, 10, 1_000).unwrap();
        let outcome = run_closing_call_auction(&mut state, 10, 2_000).unwrap();

        assert!(outcome.is_none());
        assert_eq!(state.phase, MarketPhase::ContinuousTrading);
    }

    #[test]
    fn demo_session_runs_optional_closing_auction_and_closes_market() {
        let order_book = AnyOrderBook::dense(10, 100, 16, 16);
        let mut state = EngineState::new_with_market_structure(
            [0; 16],
            7,
            order_book,
            MarketStructureConfig {
                has_opening_auction: true,
                has_closing_auction: true,
                allows_volatility_auction: false,
            },
        );

        let summary = run_demo_session(&mut state, 10, 1_000, 5_000).unwrap();

        assert_eq!(summary.opening_auction.total_count(), 2);
        assert!(summary.closing_auction.is_some());
        let closing = summary.closing_auction.unwrap();
        assert_eq!(closing.total_count(), 2);
        assert_eq!(closing.trades[0].price, 110);
        assert_eq!(closing.trades[1].price, 110);
        assert_eq!(state.phase, MarketPhase::Closed);
        assert!(state.active_auction().is_none());
    }
}
