#!/bin/sh
# Builds the browser version into docs/ (served by GitHub Pages).
set -e
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown --bin rogueball
rm -rf docs
mkdir -p docs
cp target/wasm32-unknown-unknown/release/rogueball.wasm docs/
cp web/index.html web/rogueball.js docs/
cp "$(ls -d ~/.cargo/registry/src/*/macroquad-0.4.16/js | head -1)/mq_js_bundle.js" docs/
# the art is embedded in the wasm (see build.rs)
touch docs/.nojekyll
echo "built docs/"
