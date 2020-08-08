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
    thread::spawn(|| {
        assert_eq!(bouncer::run(1, 2, 3, &dummy_function).is_ok(), true);
    });
    thread::spawn(|| {
        assert_eq!(bouncer::run(2, 3, 5, &dummy_function).is_ok(), true);
    });
    assert_eq!(bouncer::run(0, 5, 1, &dummy_function).is_ok(), true);
}
