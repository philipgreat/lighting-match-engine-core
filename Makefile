all:
	cargo run -- --prodid 7 --tag FIX009 --test-order-book-size 10k
release:
	cargo build --release
	#target/release/lighting-match-engine-core --prodid 7 --tag FIX009
