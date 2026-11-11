#!/usr/bin/env bash
# Builds Lintel for the browser into target/web. Needs the wasm target and the wasm-bindgen
# CLI at the version in Cargo.lock:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.130 --locked
# Serve target/web over HTTPS (or from localhost). The browser needs WebGPU.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release --lib --target wasm32-unknown-unknown
mkdir -p target/web
wasm-bindgen --target web --out-dir target/web --out-name app target/wasm32-unknown-unknown/release/lintel.wasm
cp web/index.html target/web/index.html
echo "Built target/web ($(gzip -c target/web/app_bg.wasm | wc -c) bytes of wasm, gzipped)"
