# Lighting Match Engine Core

**RUST IS BORN for MATCHING ENGINES**

A minized lighting fast matching engine core, focusing on mathing only.
No assumptions, work with different products with max 65535.

Products can be:

* Stock Trading: Like AAPL, TSLA, Meta
* Cryptocurrency Trading Pairs: BTC-USDT, ETH-USDT, XRP-USDT
* Futures Contracts: Like S&P 500 futures, Oil futures
* Options Trading: Like call and put options on stocks or indices
* Forex Trading: Like USD/EUR, GBP/JPY
* Commodity Trading: Like Gold, Silver, Crude Oil
* Derivatives Markets: Like CFDs (Contracts for Difference)
* NFT Markets: Like NFT auctions and buy/sell orders
* Peer-to-Peer (P2P) Trading Platforms: Like decentralized exchanges (DEXs) for token swaps
* Real Estate: Like property listing matching for rental or purchase

All the products can be encoded as a number less than 65535, 0 is reseve for online testing



## Order types

* Market order
* Limit orders

## Policies

* Prices first
* Time first

## Time and package size
* All time using nano second as the time unit, most hardware support u seconds precison, some hardware support higher precision timing.
* All package size are 50 bytes

## Why fast

* Only a single asset per running instance
* Minimize depencies only tokio for networking
* Do things that Rust is good at
* No DB
* No remote cache
* No JSON
* No file reading/writes
* No computing with strings, floats, only integeral types
* Purely in memory except rebuild the order book
* Recieving orders by UDP multicast
* Keep code lines less than 2000 （now it is 500）
* 50 bytes per package

## Why reliable

* Only tokio used as third party crates.
* Keeping less changes

## Make reliable

* Running two or more instances

## Quick start

Quick Start: Launching the Matching Engine
The simplest way to start the engine is by specifying the two required parameters: the instance tag (--name) and the product ID (--prodid).

Example Command:

```bash
./target/release/match_engine --name TFX01 --prodid 505
```

Explanation:

This command launches an engine instance with the unique identifier TFX01 (the instance tag) dedicated to matching orders for Product 505. All network communication will use the default multicast addresses (224.0.0.1:5000 for trades and 224.0.0.2:5000 for status).

Note: The instance tag (--name) must be 8 characters or less.


## How it works

* Rebuild order book from order book fuel server(order book fuel server is NOT in the project)
* Recieving order request when order book has been built
* Test match
* Broadcasting matching result

## Source Files

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

## Deployment

* In a network supporting multicasting
* Docker: with --network host
* Kubernates: [Config k8s](./docs/config-k8s-network.md)

## SDK & Testing tool

We are planning to build Java and Rust sdk. Currently not availiable yet.

Testbench is WIP.

## In scope and NOT in scope

This is not subject to do everyting, the secrets of fast and reliable is KISS(simple and stupid)

What is IN scope

* A SIMPLE, ROBUST system handling large orderbook process need many external tools to work with

External systems you may need and NOT in scope

* Product Management System to define what can be trade to present to end user and control the changes within an organzation and maps products id as a valid long type, less than 65535

* Market Data System: Provides real-time and historical market data, ensuring the trading system has accurate price feeds and market insights.

* Order Management System (OMS): Manages and routes orders to the appropriate execution venues, ensuring efficient order processing and tracking.

* Risk Management System: Monitors and enforces risk controls, ensuring trades comply with predefined limits and minimizing exposure to significant losses.

* Clearing and Settlement System: Handles the confirmation, clearing, and settlement of trades, ensuring proper transfer of funds and assets.

* Trade Surveillance and Compliance System: Monitors trading activity to detect and prevent market manipulation and ensures adherence to regulatory requirements.

* Liquidity Management System: Ensures that there is enough liquidity available to match orders quickly, preventing slippage and improving trade execution.

* Backtesting System: Tests trading strategies against historical data to evaluate their performance before deployment in a live environment.

* Data Storage and Analytics System: Stores vast amounts of trading and market data, providing powerful analytics and performance insights for strategy improvement.

* Security and Authentication System: Protects the trading platform and user data by enforcing security protocols, such as encryption and two-factor authentication.

* Payment and Wallet System (For Cryptocurrency): Manages deposits, withdrawals, and balances for digital assets, ensuring smooth transactions in cryptocurrency exchanges.

## Limitations

* Max 65535 products
* Price from 0 to 2**64
* Order id from 0 to 2**64

## License

[MIT License](./LICENSE.md)

## Contact

* Telegram: <https://t.me/philip_is_online>
* Discuss on Redit: <https://www.reddit.com/r/rust/comments/1nuxveq/i_am_build_a_performance_first_matching_engine/>