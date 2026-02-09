all:
	cargo run --features match-timing --release -- --prodid 7 --name AAPL --test-order-book-size 50k
notiming:
	cargo run  --release -- --prodid 7 --name AAPL --test-order-book-size 50k

release:
	cargo build --release
	#target/release/lighting-match-engine-core --prodid 7 --name AAPL --test-order-book-size 50k

