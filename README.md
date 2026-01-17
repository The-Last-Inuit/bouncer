<p align="center">
  <img src="bouncer.png" alt="Rupertou">
</p>

Bouncer helps you to manage your requests to third-party services.

#### Setup

Bouncer requires Redis. By default it connects to `redis://127.0.0.1/`.

You can override the Redis URL via the `BOUNCER_REDIS_URL` environment variable or by
passing a `BouncerConfig`.

The Lua script is embedded by default. If you want to supply a custom script, you can
set `BOUNCER_SCRIPT` with the script contents or build a config from a file path:

```rust
use bouncer::BouncerConfig;

let config = BouncerConfig::from_script_path(
    "redis://127.0.0.1/",
    "path/to/script.lua",
)?;
```

#### Usage

Imagine you have a function that makes a request to a service.

```rust
fn dummy_function() {
    let mut easy = Easy::new();
    easy.url("http://httpbin.org/delay/3").unwrap();
    easy.write_function(|data| {
      stdout().write_all(data).unwrap();
      Ok(data.len())
    }).unwrap();
    easy.perform().unwrap();
}
```

You can use bouncer to call that function many times in many places, and
let bouncer handle the rate limiting.

```rust
use bouncer::{Bouncer, BouncerConfig};

let config = BouncerConfig::default();

Bouncer::new(&dummy_function)
    .key("user:123")
    .rate_limit(RATE_LIMIT)
    .wait_time(WAIT_TIME)
    .run_with(&config)?;
```

If you prefer defaults, call `.run()` instead of `.run_with(&config)`.

#### Decisions and Async

If you want to make the decision yourself, use `decide_with` and check the result:

```rust
use bouncer::{Bouncer, BouncerConfig, BouncerDecision};

let config = BouncerConfig::default();
let decision = Bouncer::new(&dummy_function)
    .key("user:123")
    .rate_limit(RATE_LIMIT)
    .wait_time(WAIT_TIME)
    .decide_with(&config);

match decision {
    BouncerDecision::Allowed(_) => dummy_function(),
    BouncerDecision::Wait(stats) => std::thread::sleep(stats.wait),
    BouncerDecision::Errored(err) => return Err(err.into()),
}
```

For async contexts, use `run_async_with`:

```rust
use bouncer::{Bouncer, BouncerConfig};

let config = BouncerConfig::default();

Bouncer::new(&dummy_function)
    .key("user:123")
    .rate_limit(RATE_LIMIT)
    .wait_time(WAIT_TIME)
    .run_async_with(&config)
    .await?;
```

#### Pooling

If you want to reuse connections across calls, create a pool and pass it in:

```rust
use bouncer::{Bouncer, BouncerConfig, BouncerPool};

let config = BouncerConfig::default();
let pool = BouncerPool::from_config(&config)?;

Bouncer::new(&dummy_function)
    .key("user:123")
    .rate_limit(RATE_LIMIT)
    .wait_time(WAIT_TIME)
    .run_with_pool(&pool, &config)?;
```
