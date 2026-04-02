use crate::fix::encoder::{
    encode_cancel_report, encode_execution_report, encode_order_status_report, encode_reject,
};
use crate::fix::model::{
    build_exec_id, build_execution_reports, ExecutionType, FixIncomingMessage, NewOrderRequest,
    OrderStatusReport, RejectReport,
};
use crate::fix::parser::parse_fix_message;
use crate::matching_engine::{now_nanos, BookSnapshot, EngineStats};
use crate::module::state::ModuleState;

pub fn process_fix_message(
    state: &mut ModuleState,
    product_id: u16,
    raw_fix: &str,
) -> Result<Vec<String>, String> {
    let message = parse_fix_message(raw_fix)?;

    let replies = match message {
        FixIncomingMessage::NewOrder(request) => process_new_order(state, product_id, request),
        FixIncomingMessage::Cancel(request) => {
            let engine = state.get_or_create_engine(product_id);
            let leaves_qty = engine.remaining_quantity(request.orig_order_id);
            if engine.cancel_existing_order(request.orig_order_id) {
                state.cancel_order_status(
                    product_id,
                    request.orig_order_id,
                    request.symbol.clone(),
                    leaves_qty.unwrap_or(0),
                );
                vec![encode_cancel_report(
                    request.orig_order_id,
                    request.symbol,
                    leaves_qty.unwrap_or(0),
                )]
            } else {
                vec![encode_reject(&RejectReport {
                    exec_id: build_exec_id(request.orig_order_id, &ExecutionType::Rejected, 0),
                    order_id: Some(request.orig_order_id),
                    text: format!("cancel target not found: {}", request.orig_order_id),
                })]
            }
        }
        FixIncomingMessage::Replace(request) => {
            let engine = state.get_or_create_engine(product_id);
            if !engine.has_order(request.orig_order_id) {
                vec![encode_reject(&RejectReport {
                    exec_id: build_exec_id(request.orig_order_id, &ExecutionType::Rejected, 0),
                    order_id: Some(request.orig_order_id),
                    text: format!("replace target not found: {}", request.orig_order_id),
                })]
            } else if request.new_order_id != request.orig_order_id && engine.has_order(request.new_order_id) {
                vec![encode_reject(&RejectReport {
                    exec_id: build_exec_id(request.new_order_id, &ExecutionType::Rejected, 0),
                    order_id: Some(request.new_order_id),
                    text: format!("duplicate replacement order id {}", request.new_order_id),
                })]
            } else {
                let new_order_request = NewOrderRequest {
                    order_id: request.new_order_id,
                    side: request.side,
                    quantity: request.quantity,
                    price_type: request.price_type,
                    price: request.price,
                    symbol: request.symbol.clone(),
                    transact_time: request.transact_time,
                };

                engine.cancel_existing_order(request.orig_order_id);
                state.cancel_order_status(
                    product_id,
                    request.orig_order_id,
                    request.symbol.clone(),
                    0,
                );
                process_new_order(state, product_id, new_order_request)
            }
        }
        FixIncomingMessage::Status(request) => match state.order_status(product_id, request.order_id) {
            Some(report) => vec![encode_order_status_report(&report)],
            None => vec![encode_reject(&RejectReport {
                exec_id: build_exec_id(request.order_id, &ExecutionType::Rejected, 0),
                order_id: Some(request.order_id),
                text: format!("order status not found: {}", request.order_id),
            })],
        },
    };

    Ok(replies)
}

pub fn process_book_snapshot(state: &mut ModuleState, product_id: u16) -> BookSnapshot {
    state.get_or_create_engine(product_id).book_snapshot()
}

pub fn process_stats_snapshot(state: &mut ModuleState, product_id: u16) -> EngineStats {
    state.get_or_create_engine(product_id).stats_snapshot()
}

pub fn process_reset(state: &mut ModuleState, product_id: u16) -> bool {
    state.reset_engine(product_id)
}

fn process_new_order(
    state: &mut ModuleState,
    product_id: u16,
    request: NewOrderRequest,
) -> Vec<String> {
    let order_id = request.order_id;
    let order = request.clone().into_order(product_id, now_nanos());
    let engine = state.get_or_create_engine(product_id);
    match engine.submit_order(order) {
        Ok(outcome) => {
            let reports = build_execution_reports(&request, outcome.resting_qty, &outcome.match_result);
            if let Some(last) = reports.last() {
                state.upsert_order_status(
                    product_id,
                    OrderStatusReport {
                        order_id: last.order_id,
                        symbol: last.symbol.clone(),
                        cum_qty: last.cum_qty,
                        leaves_qty: last.leaves_qty,
                        last_qty: last.last_qty,
                        last_px: last.last_px,
                        ord_status: last.ord_status,
                        exec_type: last.exec_type.clone(),
                    },
                );
            }
            reports
                .into_iter()
                .map(|report| encode_execution_report(&report))
                .collect::<Vec<_>>()
        }
        Err(text) => vec![encode_reject(&RejectReport {
            exec_id: build_exec_id(order_id, &ExecutionType::Rejected, 0),
            order_id: Some(order_id),
            text,
        })],
    }
}

#[cfg(test)]
mod tests {
    use crate::module::state::ModuleState;

    use super::{process_book_snapshot, process_fix_message, process_reset, process_stats_snapshot};

    #[test]
    fn end_to_end_new_order_cancel_replace_flow() {
        let mut state = ModuleState::new();

        let new_reply = process_fix_message(
            &mut state,
            7,
            "35=D|11=1001|54=1|38=5|40=2|44=101|55=AAPL|",
        )
        .unwrap();
        assert_eq!(new_reply.len(), 1);
        assert!(new_reply[0].contains("150=0"));
        assert!(new_reply[0].contains("151=5"));

        let replace_reply = process_fix_message(
            &mut state,
            7,
            "35=G|11=1002|41=1001|54=1|38=8|40=2|44=102|55=AAPL|",
        )
        .unwrap();
        assert_eq!(replace_reply.len(), 1);
        assert!(replace_reply[0].contains("11=1002"));
        assert!(replace_reply[0].contains("31=102"));
        assert!(replace_reply[0].contains("151=8"));

        let cancel_reply = process_fix_message(
            &mut state,
            7,
            "35=F|11=2001|41=1002|55=AAPL|",
        )
        .unwrap();
        assert_eq!(cancel_reply.len(), 1);
        assert!(cancel_reply[0].contains("150=4"));
        assert!(cancel_reply[0].contains("151=8"));

        let status_reply = process_fix_message(&mut state, 7, "35=H|37=1002|55=AAPL|").unwrap();
        assert_eq!(status_reply.len(), 1);
        assert!(status_reply[0].contains("150=4"));
        assert!(status_reply[0].contains("39=4"));
    }

    #[test]
    fn end_to_end_trade_reports_fill_state() {
        let mut state = ModuleState::new();

        process_fix_message(
            &mut state,
            7,
            "35=D|11=2001|54=2|38=4|40=2|44=101|55=AAPL|",
        )
        .unwrap();

        let reply = process_fix_message(
            &mut state,
            7,
            "35=D|11=2002|54=1|38=4|40=2|44=101|55=AAPL|",
        )
        .unwrap();

        assert_eq!(reply.len(), 1);
        assert!(reply[0].contains("150=F"));
        assert!(reply[0].contains("39=2"));
        assert!(reply[0].contains("14=4"));
        assert!(reply[0].contains("151=0"));
        assert!(reply[0].contains("32=4"));
        assert!(reply[0].contains("31=101"));
    }

    #[test]
    fn end_to_end_multi_fill_generates_multiple_trade_reports() {
        let mut state = ModuleState::new();

        process_fix_message(
            &mut state,
            7,
            "35=D|11=3001|54=2|38=2|40=2|44=101|55=AAPL|",
        )
        .unwrap();
        process_fix_message(
            &mut state,
            7,
            "35=D|11=3002|54=2|38=3|40=2|44=101|55=AAPL|",
        )
        .unwrap();

        let reply = process_fix_message(
            &mut state,
            7,
            "35=D|11=3003|54=1|38=5|40=2|44=101|55=AAPL|",
        )
        .unwrap();

        assert_eq!(reply.len(), 2);
        assert!(reply[0].contains("17=3003-TRD-0"));
        assert!(reply[0].contains("14=2"));
        assert!(reply[0].contains("151=3"));
        assert!(reply[1].contains("17=3003-TRD-1"));
        assert!(reply[1].contains("14=5"));
        assert!(reply[1].contains("151=0"));
        assert!(reply[1].contains("39=2"));
    }

    #[test]
    fn service_exposes_book_stats_and_reset() {
        let mut state = ModuleState::new();

        process_fix_message(
            &mut state,
            7,
            "35=D|11=4001|54=1|38=6|40=2|44=110|55=AAPL|",
        )
        .unwrap();

        let book = process_book_snapshot(&mut state, 7);
        assert_eq!(book.best_bid, Some(110));
        assert_eq!(book.total_bid_volume, 6);

        let stats = process_stats_snapshot(&mut state, 7);
        assert_eq!(stats.product_id, 7);
        assert_eq!(stats.total_received_orders, 1);

        assert!(process_reset(&mut state, 7));
        let reset_book = process_book_snapshot(&mut state, 7);
        assert_eq!(reset_book.best_bid, None);
        assert_eq!(reset_book.total_bid_volume, 0);
    }

    #[test]
    fn status_request_returns_reject_for_unknown_order() {
        let mut state = ModuleState::new();
        let reply = process_fix_message(&mut state, 7, "35=H|37=9999|55=AAPL|").unwrap();

        assert_eq!(reply.len(), 1);
        assert!(reply[0].contains("150=8"));
        assert!(reply[0].contains("order status not found: 9999"));
    }
}
