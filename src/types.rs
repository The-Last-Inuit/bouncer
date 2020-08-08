use serde::Deserialize;
#[derive(Debug, Deserialize, PartialEq)]
pub struct BouncerStats {
    pub old: String,
    pub current: String,
    pub since: u8,
    pub wait: Vec<u32>,
}

impl Default for BouncerStats {
    fn default() -> BouncerStats {
        BouncerStats {
            current: 0.to_string(),
            old: 0.to_string(),
            since: 0,
            wait: vec![0],
        }
    }
}

use redis::{FromRedisValue, RedisResult, Value};
use std::str::from_utf8;
impl FromRedisValue for BouncerStats {
    fn from_redis_value(v: &Value) -> RedisResult<BouncerStats> {
        let rv: RedisResult<BouncerStats> = match *v {
            Value::Data(ref d) => {
                let result = serde_json::from_str(from_utf8(d)?);
                let v: BouncerStats = match result {
                    Ok(value) => value,
                    Err(_) => BouncerStats::default(),
                };
                Ok(v)
            }
            _ => invalid_type_error!(v, "Not JSON compatible"),
        };
        match rv {
            Ok(value) => Ok(value),
            Err(_) => invalid_type_error!(v, "Not valid JSON"),
        }
    }
}
