use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

fn dummy_function() {
    CALL_COUNT.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn it_should_run() {
    use bouncer::{Bouncer, BouncerConfig};

    CALL_COUNT.store(0, Ordering::SeqCst);

    let config = Arc::new(BouncerConfig::default());
    let cases = [
        ("key-1", 2u8, 3u8),
        ("key-2", 3u8, 5u8),
        ("key-0", 5u8, 1u8),
    ];
    let mut handles = Vec::new();

    for (key, rate_limit, wait_time) in cases.iter().copied() {
        let config = Arc::clone(&config);
        handles.push(thread::spawn(move || {
            assert!(
                Bouncer::new(&dummy_function)
                    .key(key)
                    .rate_limit(rate_limit)
                    .wait_time(wait_time)
                    .run_with(config.as_ref())
                    .is_ok()
            );
        }));
    }

    for handle in handles {
        handle.join().expect("thread join");
    }

    assert_eq!(CALL_COUNT.load(Ordering::SeqCst), cases.len());
}
