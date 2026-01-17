use redis::{
    aio::Connection as AsyncConnection, Client, FromRedisValue, RedisError, RedisResult, Script,
    Value,
};
use serde::Deserialize;
use std::env;
use std::fs::read_to_string;
use std::path::Path;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tokio::time::delay_for;

const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1/";
const DEFAULT_SCRIPT: &str = include_str!("script.lua");

#[derive(Debug, Clone)]
pub struct BouncerConfig {
    pub redis_url: String,
    pub script: String,
}

impl BouncerConfig {
    /// Creates a config with the provided Redis URL and Lua script contents.
    pub fn new(redis_url: impl Into<String>, script: impl Into<String>) -> Self {
        Self {
            redis_url: redis_url.into(),
            script: script.into(),
        }
    }

    /// Creates a config by reading the Lua script from a file path.
    pub fn from_script_path(
        redis_url: impl Into<String>,
        script_path: impl AsRef<Path>,
    ) -> std::io::Result<Self> {
        let script = read_to_string(script_path)?;
        Ok(Self::new(redis_url, script))
    }
}

impl Default for BouncerConfig {
    fn default() -> Self {
        let redis_url =
            env::var("BOUNCER_REDIS_URL").unwrap_or_else(|_| DEFAULT_REDIS_URL.to_string());
        let script = env::var("BOUNCER_SCRIPT").unwrap_or_else(|_| DEFAULT_SCRIPT.to_string());
        Self { redis_url, script }
    }
}

#[derive(Debug)]
pub enum BouncerDecision {
    Allowed(BouncerStats),
    Wait(BouncerStats),
    Errored(RedisError),
}

impl BouncerDecision {
    pub fn stats(&self) -> Option<&BouncerStats> {
        match self {
            BouncerDecision::Allowed(stats) | BouncerDecision::Wait(stats) => Some(stats),
            BouncerDecision::Errored(_) => None,
        }
    }
}

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

#[derive(Debug, Deserialize, PartialEq, Default)]
pub struct BouncerStats {
    #[serde(default)]
    pub allowed: bool,
    #[serde(default)]
    pub old: u64,
    #[serde(default)]
    pub current: u64,
    #[serde(default)]
    pub since: u64,
    #[serde(default)]
    pub wait: Duration,
}

impl BouncerStats {
    fn is_allowed(&self) -> bool {
        self.allowed
            || (self.wait == Duration::from_secs(0)
                && self.since == 0
                && self.old == 0
                && self.current == 0)
    }
}

impl FromRedisValue for BouncerStats {
    fn from_redis_value(v: &Value) -> RedisResult<BouncerStats> {
        match *v {
            Value::Data(ref d) => {
                serde_json::from_slice(d).map_err(|_| Error::InvalidJson(v).into())
            }
            _ => Err(Error::IncompatibleJson(v).into()),
        }
    }
}

#[derive(Clone)]
pub struct Bouncer<'a> {
    pub key: String,
    pub rate_limit: u8,
    pub wait_time: u8,
    pub block: &'a dyn Fn(),
}

impl<'a> Default for Bouncer<'a> {
    /// Default values for a Bouncer instance.
    fn default() -> Bouncer<'a> {
        Bouncer {
            key: "default".to_string(),
            rate_limit: 5,
            wait_time: 2,
            block: &|| println!("Nothing to excucute"),
        }
    }
}

pub struct BouncerPool {
    client: Client,
    connections: Mutex<Vec<redis::Connection>>,
}

impl BouncerPool {
    pub fn new(redis_url: impl Into<String>) -> RedisResult<Self> {
        let client = Client::open(redis_url.into())?;
        Ok(Self {
            client,
            connections: Mutex::new(Vec::new()),
        })
    }

    pub fn from_config(config: &BouncerConfig) -> RedisResult<Self> {
        Self::new(config.redis_url.clone())
    }

    fn with_connection<T>(
        &self,
        f: impl FnOnce(&mut redis::Connection) -> RedisResult<T>,
    ) -> RedisResult<T> {
        let maybe_connection = {
            let mut connections = self.connections.lock().expect("bouncer pool lock");
            connections.pop()
        };
        let mut connection = match maybe_connection {
            Some(connection) => connection,
            None => self.client.get_connection()?,
        };

        let result = f(&mut connection);
        if result.is_ok() {
            let mut connections = self.connections.lock().expect("bouncer pool lock");
            connections.push(connection);
        }
        result
    }
}

impl<'a> Bouncer<'a> {
    /// Returns a Bouncer instance
    ///
    /// # Arguments
    ///
    /// * `block` - A block or a function that will execute a request to a third-party service.
    pub fn new(block: &'a dyn Fn()) -> Bouncer<'a> {
        let mut bouncer = Bouncer::default();
        bouncer.block = block;
        bouncer
    }

    /// Assigns a key to the process to be executed.
    ///
    /// # Arguments
    ///
    /// * `key` - A identifier to identify the process.
    pub fn key(&mut self, key: impl Into<String>) -> &mut Self {
        self.key = key.into();
        self
    }

    /// Assigns a rate limit to the process to be executed.
    ///
    /// # Arguments
    ///
    /// * `rate_limit` - limit the number of calls.
    pub fn rate_limit(&mut self, rate_limit: u8) -> &mut Self {
        self.rate_limit = rate_limit;
        self
    }

    /// Assigns a waiting time to the process to be executed.
    ///
    /// # Arguments
    ///
    /// * `wait_time` - A value in seconds to wait until the next request to be executed.
    pub fn wait_time(&mut self, wait_time: u8) -> &mut Self {
        self.wait_time = wait_time;
        self
    }

    /// Executes the passed block with the default configuration.
    pub fn run(&self) -> RedisResult<BouncerStats> {
        self.run_with(&BouncerConfig::default())
    }

    /// Executes the passed block using the provided configuration.
    pub fn run_with(&self, config: &BouncerConfig) -> RedisResult<BouncerStats> {
        let script = Script::new(config.script.as_str());
        let client = Client::open(config.redis_url.as_str())?;
        let mut connection = client.get_connection()?;
        loop {
            let bouncer_stats = self.invoke(&script, &mut connection)?;
            if bouncer_stats.is_allowed() {
                (self.block)();
                return Ok(bouncer_stats);
            }
            thread::sleep(bouncer_stats.wait);
        }
    }

    /// Executes the passed block using a pooled connection.
    pub fn run_with_pool(
        &self,
        pool: &BouncerPool,
        config: &BouncerConfig,
    ) -> RedisResult<BouncerStats> {
        loop {
            match self.decide_with_pool(pool, config) {
                BouncerDecision::Allowed(stats) => {
                    (self.block)();
                    return Ok(stats);
                }
                BouncerDecision::Wait(stats) => thread::sleep(stats.wait),
                BouncerDecision::Errored(err) => return Err(err),
            }
        }
    }

    /// Executes the passed block with the default configuration without blocking threads.
    pub async fn run_async(&self) -> RedisResult<BouncerStats> {
        self.run_async_with(&BouncerConfig::default()).await
    }

    /// Executes the passed block with the provided configuration without blocking threads.
    pub async fn run_async_with(&self, config: &BouncerConfig) -> RedisResult<BouncerStats> {
        let script = Script::new(config.script.as_str());
        let client = Client::open(config.redis_url.as_str())?;
        let mut connection = client.get_async_connection().await?;
        loop {
            let bouncer_stats = self.invoke_async(&script, &mut connection).await?;
            if bouncer_stats.is_allowed() {
                (self.block)();
                return Ok(bouncer_stats);
            }
            delay_for(bouncer_stats.wait).await;
        }
    }

    /// Runs a single decision with the default configuration.
    pub fn decide(&self) -> BouncerDecision {
        self.decide_with(&BouncerConfig::default())
    }

    /// Runs a single decision with the provided configuration.
    pub fn decide_with(&self, config: &BouncerConfig) -> BouncerDecision {
        let result = (|| -> RedisResult<BouncerStats> {
            let script = Script::new(config.script.as_str());
            let client = Client::open(config.redis_url.as_str())?;
            let mut connection = client.get_connection()?;
            self.invoke(&script, &mut connection)
        })();

        match result {
            Ok(stats) => self.stats_to_decision(stats),
            Err(err) => BouncerDecision::Errored(err),
        }
    }

    /// Runs a single decision using a pooled connection.
    pub fn decide_with_pool(&self, pool: &BouncerPool, config: &BouncerConfig) -> BouncerDecision {
        let script = Script::new(config.script.as_str());
        match pool.with_connection(|connection| self.invoke(&script, connection)) {
            Ok(stats) => self.stats_to_decision(stats),
            Err(err) => BouncerDecision::Errored(err),
        }
    }

    /// Runs a single decision with the default configuration without blocking threads.
    pub async fn decide_async(&self) -> BouncerDecision {
        self.decide_async_with(&BouncerConfig::default()).await
    }

    /// Runs a single decision with the provided configuration without blocking threads.
    pub async fn decide_async_with(&self, config: &BouncerConfig) -> BouncerDecision {
        let result = (|| async {
            let script = Script::new(config.script.as_str());
            let client = Client::open(config.redis_url.as_str())?;
            let mut connection = client.get_async_connection().await?;
            self.invoke_async(&script, &mut connection).await
        })()
        .await;

        match result {
            Ok(stats) => self.stats_to_decision(stats),
            Err(err) => BouncerDecision::Errored(err),
        }
    }

    fn stats_to_decision(&self, stats: BouncerStats) -> BouncerDecision {
        if stats.is_allowed() {
            BouncerDecision::Allowed(stats)
        } else {
            BouncerDecision::Wait(stats)
        }
    }

    fn invoke(
        &self,
        script: &Script,
        connection: &mut redis::Connection,
    ) -> RedisResult<BouncerStats> {
        script
            .key(self.key.as_str())
            .arg(self.rate_limit)
            .arg(self.wait_time)
            .invoke(connection)
    }

    async fn invoke_async(
        &self,
        script: &Script,
        connection: &mut AsyncConnection,
    ) -> RedisResult<BouncerStats> {
        script
            .key(self.key.as_str())
            .arg(self.rate_limit)
            .arg(self.wait_time)
            .invoke_async(connection)
            .await
    }
}
