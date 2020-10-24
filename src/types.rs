use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;
use std::path::PathBuf;
#[derive(Debug, Deserialize)]
struct RedisConfig {
    server_url: String,
    script: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Settings {
    debug: bool,
    redis: RedisConfig,
}

impl Settings {
    fn new() -> Result<Self, ConfigError> {
        let mut s = Config::new();
        s.merge(File::with_name("config/default"))?;
        let env = env::var("RUN_MODE").unwrap_or("development".into());
        s.merge(File::with_name(&format!("config/{}", env)).required(false))?;
        s.merge(File::with_name("config/local").required(false))?;
        s.merge(Environment::with_prefix("app"))?;
        s.try_into()
    }

    fn redis_script(&self) -> PathBuf {
        self.redis.script.clone()
    }

    fn redis_url(&self) -> String {
        self.redis.server_url.clone()
    }
}

use redis::RedisError;
#[derive(Debug)]
enum Error<'a> {
    IncompatibleJson(&'a Value),
    InvalidJson(&'a Value),
}

impl<'a> From<Error<'a>> for RedisError {
    fn from(v: Error) -> RedisError {
        match v {
            Error::IncompatibleJson(m) => RedisError::from((
                redis::ErrorKind::TypeError,
                "Response was of incompatible type",
                format!("Not JSON compatible (response was {:?})", m),
            )),
            Error::InvalidJson(m) => RedisError::from((
                redis::ErrorKind::TypeError,
                "Response was of incompatible type",
                format!("Not valid JSON (response was {:?})", m),
            )),
        }
    }
}

use std::time::Duration;
#[derive(Debug, Deserialize, PartialEq)]
pub struct BouncerStats {
    old: String,
    current: String,
    since: u8,
    #[serde(deserialize_with = "wait_deserializer")]
    wait: Duration,
}

use serde::Deserializer;
pub fn wait_deserializer<'de, D>(d: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    Deserialize::deserialize(d).map(|x: Option<_>| x.unwrap_or(Duration::from_millis(100)))
}

impl Default for BouncerStats {
    fn default() -> BouncerStats {
        BouncerStats {
            current: 0.to_string(),
            old: 0.to_string(),
            since: 0,
            wait: Duration::from_millis(0),
        }
    }
}

use redis::{FromRedisValue, Value};
use std::result::Result;
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
            _ => Result::Err(Error::IncompatibleJson(v))?,
        };
        match rv {
            Ok(value) => Ok(value),
            Err(_) => Result::Err(Error::InvalidJson(v))?,
        }
    }
}

use redis::{Client, RedisResult, Script};
use std::fs::read_to_string;
use std::thread;
#[derive(Clone)]
pub struct Bouncer<'a> {
    pub key: u8,
    pub rate_limit: u8,
    pub wait_time: u8,
    pub block: &'a dyn Fn(),
}

impl<'a> Default for Bouncer<'a> {
    /// Default values for a Bouncer instance.
    fn default() -> Bouncer<'a> {
        Bouncer {
            key: 1,
            rate_limit: 5,
            wait_time: 2,
            block: &|| println!("Nothing to excucute"),
        }
    }
}

impl<'a> Bouncer<'a> {
    /// Returns a Bouncer instance
    ///
    /// # Arguments
    ///
    /// * `block` - A block or a function that will execute a request to a third-party service.
    pub fn new(block: &'a dyn Fn()) -> Bouncer {
        let mut bouncer = Bouncer::default();
        bouncer.block = block;
        bouncer
    }

    /// Assigns a key to the process to be executed.
    ///
    /// # Arguments
    ///
    /// * `key` - A integer to identify the process.
    pub fn key(&'a mut self, key: u8) -> &'a mut Bouncer {
        self.key = key;
        self
    }

    /// Assigns a rate limit to the process to be executed.
    ///
    /// # Arguments
    ///
    /// * `rate_limit` - limit the number of calls.
    pub fn rate_limit(&'a mut self, rate_limit: u8) -> &'a mut Bouncer {
        self.rate_limit = rate_limit;
        self
    }

    /// Assigns a waiting time to the process to be executed.
    ///
    /// # Arguments
    ///
    /// * `wait_time` - A value in seconds to wait until the next request to be executed.
    pub fn wait_time(&'a mut self, wait_time: u8) -> &'a mut Bouncer {
        self.wait_time = wait_time;
        self
    }

    /// Executes the passed block.
    pub fn run(&self) -> RedisResult<BouncerStats> {
        match Settings::new() {
            Ok(settings) => {
                let raw_script = read_to_string(settings.redis_script())?;
                let script = Script::new(raw_script.as_str());
                let client = Client::open(settings.redis_url())?;
                let mut connection = client.get_connection()?;
                loop {
                    let bouncer_stats: BouncerStats = script
                        .key(self.key)
                        .arg(self.rate_limit)
                        .arg(self.wait_time)
                        .invoke(&mut connection)
                        .unwrap();
                    if bouncer_stats == BouncerStats::default() {
                        (self.block)();
                        break Ok(bouncer_stats);
                    } else {
                        thread::sleep(bouncer_stats.wait)
                    }
                }
            }
            Err(error) => panic!(error),
        }
    }
}
