.PHONY: help build check test fmt fmt-check lint example clean

help:
	@printf "Targets:\n"
	@printf "  build       Build the crate\n"
	@printf "  check       Type-check the crate\n"
	@printf "  test        Run tests (requires Redis at redis://127.0.0.1/)\n"
	@printf "  fmt         Format code with rustfmt\n"
	@printf "  fmt-check   Check formatting without modifying files\n"
	@printf "  lint        Run clippy (if installed)\n"
	@printf "  example     Run the async Redis example\n"
	@printf "  clean       Remove build artifacts\n"

build:
	cargo build

check:
	cargo check

test:
	cargo test

fmt:
	cargo fmt

fmt-check:
	cargo fmt -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

example:
	cargo run --example redis_async

clean:
	cargo clean
