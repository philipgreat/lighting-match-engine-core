use crate::config::{OrderBookConfig, OrderBookKind};
use crate::types::AnyOrderBook;

pub fn build_order_book(config: OrderBookConfig) -> AnyOrderBook {
    match config.kind {
        OrderBookKind::Dense => AnyOrderBook::dense(
            config.tick,
            config.base_price,
            config.max_levels,
            config.trade_cap,
        ),
        OrderBookKind::Sparse => AnyOrderBook::sparse(
            config.tick,
            config.base_price,
            config.max_levels,
            config.trade_cap,
        ),
    }
}
