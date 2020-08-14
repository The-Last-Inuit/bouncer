use curl::easy::Easy;
use std::io::{stdout, Write};
use std::thread;

fn dummy_function() {
    let mut easy = Easy::new();
    easy.url("http://httpbin.org/delay/3").unwrap();
    easy.write_function(|data| {
        stdout().write_all(data).unwrap();
        Ok(data.len())
    })
    .unwrap();
    easy.perform().unwrap();
}

#[test]
fn it_should_run() {
    use bouncer::Bouncer;
    thread::spawn(|| {
        assert_eq!(
            Bouncer::new(&dummy_function)
                .key(1)
                .rate_limit(2)
                .wait_time(3)
                .run()
                .is_ok(),
            true
        );
    });
    thread::spawn(|| {
        assert_eq!(
            Bouncer::new(&dummy_function)
                .key(2)
                .rate_limit(3)
                .wait_time(5)
                .run()
                .is_ok(),
            true
        );
    });
    assert_eq!(
        Bouncer::new(&dummy_function)
            .key(0)
            .rate_limit(5)
            .wait_time(1)
            .run()
            .is_ok(),
        true
    );
}
