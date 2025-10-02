all:
	cargo run -- --prodid 7 --tag FIX009
release:
	cargo build --release
