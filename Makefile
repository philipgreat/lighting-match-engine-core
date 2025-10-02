all:
	cargo run -- --prodid 7 --tag FIX009
release:
	cargo build --release
	#target/release/lighting-match-engine-core --prodid 7 --tag FIX009
