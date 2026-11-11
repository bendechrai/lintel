#!/usr/bin/env bash
# Builds the site into target/web: the landing page, and Lintel for the browser under app/. Needs the wasm target and the wasm-bindgen
# CLI at the version in Cargo.lock:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.130 --locked
# Serve target/web over HTTPS (or from localhost). The browser needs WebGPU.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release --lib --target wasm32-unknown-unknown
# The landing page is the site's front door; the app lives under app/.
rm -rf target/web
mkdir -p target/web/app
cp -r site/. target/web/
wasm-bindgen --target web --out-dir target/web/app --out-name app target/wasm32-unknown-unknown/release/lintel.wasm
cp web/index.html target/web/app/index.html
echo "Built target/web ($(gzip -c target/web/app/app_bg.wasm | wc -c) bytes of wasm, gzipped)"
