#[cfg(feature = "redis-module-host")]
use redis_module::{redis_module, Context, NextArg, RedisError, RedisResult, RedisString, RedisValue};
#[cfg(feature = "redis-module-host")]
use redis_module::redisvalue::RedisValueKey;
#[cfg(feature = "redis-module-host")]
use std::collections::BTreeMap;

#[cfg(feature = "redis-module-host")]
use crate::fix::service::{
    process_book_snapshot, process_fix_message, process_reset, process_stats_snapshot,
};
#[cfg(feature = "redis-module-host")]
use crate::module::state::global_state;

#[cfg(feature = "redis-module-host")]
fn parse_product_id(arg: &RedisString) -> Result<u16, RedisError> {
    arg.try_as_str()?
        .parse::<u16>()
        .map_err(|_| RedisError::String("product_id must be a valid u16".to_string()))
}

#[cfg(feature = "redis-module-host")]
fn lock_state() -> Result<std::sync::MutexGuard<'static, crate::module::state::ModuleState>, RedisError> {
    global_state()
        .lock()
        .map_err(|_| RedisError::String("module state lock poisoned".to_string()))
}

#[cfg(feature = "redis-module-host")]
fn fix_send(_ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    let mut args = args.into_iter().skip(1);
    let product_id = parse_product_id(&args.next_arg()?)?;
    let raw_fix = args.next_string()?;
    let mut state = lock_state()?;
    let replies = process_fix_message(&mut state, product_id, &raw_fix)
        .map_err(|err| RedisError::String(format!("FIX processing error: {err}")))?;

    Ok(RedisValue::Array(
        replies.into_iter().map(RedisValue::SimpleString).collect(),
    ))
}

#[cfg(feature = "redis-module-host")]
fn fix_book(_ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    let mut args = args.into_iter().skip(1);
    let product_id = parse_product_id(&args.next_arg()?)?;
    let mut state = lock_state()?;
    let snapshot = process_book_snapshot(&mut state, product_id);

    let mut map = BTreeMap::new();
    map.insert(RedisValueKey::String("product_id".to_string()), RedisValue::Integer(snapshot.product_id as i64));
    map.insert(
        RedisValueKey::String("best_bid".to_string()),
        snapshot.best_bid.map(|v| RedisValue::Integer(v as i64)).unwrap_or(RedisValue::Null),
    );
    map.insert(
        RedisValueKey::String("best_ask".to_string()),
        snapshot.best_ask.map(|v| RedisValue::Integer(v as i64)).unwrap_or(RedisValue::Null),
    );
    map.insert(RedisValueKey::String("bid_levels".to_string()), RedisValue::Integer(snapshot.bid_levels as i64));
    map.insert(RedisValueKey::String("ask_levels".to_string()), RedisValue::Integer(snapshot.ask_levels as i64));
    map.insert(
        RedisValueKey::String("total_bid_volume".to_string()),
        RedisValue::Integer(snapshot.total_bid_volume as i64),
    );
    map.insert(
        RedisValueKey::String("total_ask_volume".to_string()),
        RedisValue::Integer(snapshot.total_ask_volume as i64),
    );

    Ok(RedisValue::OrderedMap(map))
}

#[cfg(feature = "redis-module-host")]
fn fix_stats(_ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    let mut args = args.into_iter().skip(1);
    let product_id = parse_product_id(&args.next_arg()?)?;
    let mut state = lock_state()?;
    let stats = process_stats_snapshot(&mut state, product_id);

    Ok(RedisValue::Array(vec![
        RedisValue::Integer(stats.product_id as i64),
        RedisValue::Integer(stats.total_received_orders as i64),
        RedisValue::Integer(stats.matched_orders as i64),
        RedisValue::Integer(stats.start_time as i64),
    ]))
}

#[cfg(feature = "redis-module-host")]
fn fix_reset(_ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    let mut args = args.into_iter().skip(1);
    let product_id = parse_product_id(&args.next_arg()?)?;
    let mut state = lock_state()?;
    let removed = process_reset(&mut state, product_id);
    Ok(RedisValue::SimpleString(if removed {
        "OK".to_string()
    } else {
        "NOOP".to_string()
    }))
}

#[cfg(all(feature = "redis-module-host", not(test)))]
redis_module! {
    name: "lighting_match_engine",
    version: 1,
    allocator: (redis_module::alloc::RedisAlloc, redis_module::alloc::RedisAlloc),
    data_types: [],
    commands: [
        ["fix.send", fix_send, "write", 0, 0, 0],
        ["fix.book", fix_book, "readonly", 0, 0, 0],
        ["fix.stats", fix_stats, "readonly", 0, 0, 0],
        ["fix.reset", fix_reset, "write", 0, 0, 0],
    ],
}
