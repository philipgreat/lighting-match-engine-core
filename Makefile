all:
	cargo run -- --prod-id 7 --tag FIX009 --test-order-book-size 10k
release:
	cargo build --release
	target/release/lighting-match-engine-core --prod-id 7 --tag FIX009 --test-order-book-size 10k
