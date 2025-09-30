# Ligting Match Engine Core

A lighting fast matching engine core, focusing on mathing only.

No assumptions, work with different assets

## Why fast

* Minize depency only tokio for networking
* Do things that Rust is good at
* No DB
* No remote cache
* No computing with strings, floats, only intergral types
* Purely in memory except rebuild the order book
* Recieving orders by UDP multicast
* Keep code lines less than 2000 （now it is 200）
* 40 bytes per package

## Make reliable

* Running two or more instances

## How it works

* Rebuild order book
* Recieving order request when order book has been built
* Test match
* Broadcasting matching result

