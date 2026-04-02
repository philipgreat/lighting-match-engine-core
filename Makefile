all:
	mkdir -p perf-data
	cargo run --features match-timing --release -- --prodid 7 --name AAPL --test-order-book-size 50k
notiming:
	cargo run  --release -- --prodid 7 --name AAPL --test-order-book-size 50k

release:
	cargo build --release
	#target/release/lighting-match-engine-core --prodid 7 --name AAPL --test-order-book-size 50k

redis-module-release:
	cargo build --release --features redis-module-host

redis-module-run:
	/opt/homebrew/bin/redis-server --port 6380 --loadmodule $(CURDIR)/target/release/liblighting_match_engine_core.dylib
