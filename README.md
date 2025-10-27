### Uniswap V4 Pool Price Tracker

A Rust-based real-time tracker built with Alloy that listens to Uniswap V4 swap events over WebSocket and derives token prices from each event's `sqrtPriceX96` value.

**This project is for practice and learning purposes only; use it carefully and do not rely on it in production or critical environments.**

## Features

- Connects to Ethereum mainnet through WebSocket (Alchemy or Public Node)

- Dynamically loads multiple pool IDs from a .env file

- Listens to Swap events from multiple Uniswap V4 pools concurrently

- Fetches pool token addresses, symbols, and decimals

- Calculates price ratios from sqrtPriceX96

- Prints real-time token-to-token prices

## Requirements

- Rust

- Cargo

## Installation

Clone the repository:

`git clone https://github.com/dlr-a/uniswapv4-pool-price-tracker.git`

`cd uniswapv4-pool-price-tracker`

`cargo build`

## Environment Configuration

Create a .env file in the project root and add your pool IDs like:

`POOL_IDS=poolId1,poolId2`

Each ID should be separated by commas.

## Run the tracker using Cargo

Start the project using Cargo:

`cargo run`

## Notes

- By default, the tracker connects to wss://ethereum-rpc.publicnode.com.

- You can replace the RPC URL with your own provider (e.g., Alchemy).
