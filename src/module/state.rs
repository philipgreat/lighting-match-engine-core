use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::data_types::EngineState;
use crate::fix::model::{ExecutionType, OrderStatusReport};
use crate::matching_engine::tag_to_u16_array;

pub struct ModuleState {
    engines: HashMap<u16, EngineState>,
    order_status: HashMap<(u16, u64), OrderStatusReport>,
}

impl ModuleState {
    pub fn new() -> Self {
        Self {
            engines: HashMap::new(),
            order_status: HashMap::new(),
        }
    }

    pub fn get_or_create_engine(&mut self, product_id: u16) -> &mut EngineState {
        self.engines
            .entry(product_id)
            .or_insert_with(|| EngineState::new_for_redis_module(tag_to_u16_array("REDIS-MODULE"), product_id))
    }

    pub fn reset_engine(&mut self, product_id: u16) -> bool {
        self.order_status.retain(|(pid, _), _| *pid != product_id);
        self.engines.remove(&product_id).is_some()
    }

    pub fn upsert_order_status(&mut self, product_id: u16, report: OrderStatusReport) {
        self.order_status.insert((product_id, report.order_id), report);
    }

    pub fn order_status(&self, product_id: u16, order_id: u64) -> Option<OrderStatusReport> {
        self.order_status.get(&(product_id, order_id)).cloned()
    }

    pub fn cancel_order_status(
        &mut self,
        product_id: u16,
        order_id: u64,
        symbol: Option<String>,
        leaves_qty: u32,
    ) -> Option<OrderStatusReport> {
        let existing = self.order_status.get(&(product_id, order_id)).cloned()?;
        let canceled = OrderStatusReport {
            order_id,
            symbol: symbol.or(existing.symbol),
            cum_qty: existing.cum_qty,
            leaves_qty,
            last_qty: 0,
            last_px: existing.last_px,
            ord_status: "4",
            exec_type: ExecutionType::Canceled,
        };
        self.upsert_order_status(product_id, canceled.clone());
        Some(canceled)
    }
}

static MODULE_STATE: OnceLock<Mutex<ModuleState>> = OnceLock::new();

pub fn global_state() -> &'static Mutex<ModuleState> {
    MODULE_STATE.get_or_init(|| Mutex::new(ModuleState::new()))
}
