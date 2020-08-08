use config::{Config, ConfigError, Environment, File};
use std::env;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RedisConfig {
    server_url: String,
    script: String,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    debug: bool,
    redis: RedisConfig,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = Config::new();
        s.merge(File::with_name("config/default"))?;
        let env = env::var("RUN_MODE").unwrap_or("development".into());
        s.merge(File::with_name(&format!("config/{}", env)).required(false))?;
        s.merge(File::with_name("config/local").required(false))?;
        s.merge(Environment::with_prefix("app"))?;
        s.try_into()
    }

    pub fn debug(&self) -> bool {
        self.debug
    }

    pub fn redis_script(&self) -> String {
      self.redis.script.clone()
    }

    pub fn redis_url(&self) -> String {
      self.redis.server_url.clone()
    }
}
