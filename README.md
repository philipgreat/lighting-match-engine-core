# Lighting Match Engine Core

A minized lighting fast matching engine core, focusing on mathing only.
No assumptions, work with different assets with max 65535 assets

## In scope and NOT in scope

This is not subject to do everyting, the secrets of fast and reliable is KISS(simple and stupid)

External systems you may need and NOT in scope

* Product Management System to define what can be trade to present to end user and control the changes within an organzation and maps asset id as a valid long type, less than 65535

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

What is IN scope

* A SIMPLE, ROBUST system handling large orderbook process need many external tools to work with

## Limitations

* Max 65535 assets
* Price from 0 to 2**64
* Order id from 0 to 2**64

## Order types

* Market order
* Limit orders

## Policies

* Prices first
* Time first

## Why fast

* Only a single asset per running instance
* Minimize depencies only tokio for networking
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

* Only tokio used as third party crates.
* Keeping less changes
  
## Make reliable

* Running two or more instances


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

## Contact

* Telegram： https://t.me/philip_is_online



