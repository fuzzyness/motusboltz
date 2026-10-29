#!/usr/bin/env bash
# Builds the WebAssembly version and serves it locally for testing.
#
# Usage: web/serve.sh [port]    (default port 8000)
#
# The site is assembled in target/web, so the .wasm never lands in web/.
# It is served on 127.0.0.1 only; open http://localhost:<port> to test.
set -euo pipefail

readonly DEFAULT_PORT=8000
readonly TARGET=wasm32-unknown-unknown

port="${1:-$DEFAULT_PORT}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
site="$root/target/web"
cd "$root"

if ! rustup target list --installed | grep -qx "$TARGET"; then
    echo "Installing the $TARGET target..."
    rustup target add "$TARGET"
fi

cargo build --release --target "$TARGET"

rm -rf "$site"
mkdir -p "$site"
# All of web/, folders such as fonts/ included, except this script and any
# stray build output.
cp -R web/. "$site/"
rm -f "$site/serve.sh" "$site"/*.wasm
cp "target/$TARGET/release/motusboltz.wasm" "$site/"

echo "Serving $site at http://localhost:$port (Ctrl-C to stop)"
exec python3 -m http.server "$port" --bind 127.0.0.1 --directory "$site"
