#!/usr/bin/env sh
set -eu

echo "Running Rust formatting check..."
cargo fmt --all -- --check

echo "Running Rust clippy..."
cargo clippy --all-targets --all-features -- -D warnings

echo "Running Rust tests..."
cargo test --all-targets --all-features

echo "Running Rust build..."
cargo build --all-targets --all-features

echo "All checks completed."
