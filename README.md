# Ligting Match Engine Core

A lighting fast matching engine core, focusing on mathing only.

No assumptions, work with different assets

## Why fast

* Minize depencies only tokio for networking
* Do things that Rust is good at
* No DB
* No remote cache
* No JSON
* No file reading/writes
* No computing with strings, floats, only intergral types
* Purely in memory except rebuild the order book
* Recieving orders by UDP multicast
* Keep code lines less than 2000 （now it is 500）
* 50 bytes per package

## Why reliable

* Running two or more instances
* Only tokio used as third party crates.
  

## How it works

* Rebuild order book from order book fuel server(order book fuel server is NOT in the project)
* Recieving order request when order book has been built
* Test match
* Broadcasting matching result

## Files

* main.rs: the entry point
* network_handler: handling communication
* data_type: data types used in the engine
* engine_stats.rs: holding order and broad casting stats
* order_matcher.rs: the core logic of matching orders
* message_codec.rs: encode/decode network messages
* broadcast_handler.rs: broadcast messages  

## How network used

* UDP: getting orders, broading casting engine stats
* TCP: use for order book rebuilding only

