mod macros;
mod types;

use redis::{Client, RedisResult, Script};
use std::fs::File;
use std::io::prelude::*;
use std::{thread, time};
use types::BouncerStats;
mod settings;
use settings::Settings;
pub fn run(key: u64, rate_limit: u8, wait_time: u8, b: &dyn Fn()) -> RedisResult<BouncerStats> {
    match Settings::new() {
        Ok(settings) => {
            let mut file = File::open(settings.redis_script())?;
            let mut script = String::new();
            file.read_to_string(&mut script)?;
            let script = Script::new(script.as_str());
            let client = Client::open(settings.redis_url())?;
            let mut connection = client.get_connection()?;
            loop {
                let bouncer_stats: BouncerStats = script
                    .key(key)
                    .arg(rate_limit)
                    .arg(wait_time)
                    .invoke(&mut connection)
                    .unwrap();
                if bouncer_stats.wait.len() == 1 {
                    b();
                    break Ok(bouncer_stats);
                } else {
                    let seconds = bouncer_stats.wait[0];
                    let microseconds = bouncer_stats.wait[1] / 1_000_000;
                    let duration = time::Duration::from_millis((seconds + microseconds) as u64);
                    thread::sleep(duration)
                }
            }
        }
        Err(error) => panic!(error),
    }
}
