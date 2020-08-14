<p align="center">
  <img src="bouncer.png" alt="Rupertou">
</p>

Bouncer helps you to manage your requests to third-party services.

#### Setup

In order to use Bouncer, you need Redis as a backend. Then you can configure Redis URI:

```
#config/default.toml

server_url="redis://redis/"
```

#### Usage

Imagine you have a function that makes a request to a services.

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

You can use bouncer to call that function many times in many place i.e. asynch jobs and 
let bouncer handle the request.

```rust
bouncer::run(KEY, RATE_LIMIT, WAIT_TIME, &dummy_function)
```
