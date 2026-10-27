#!/usr/bin/env bash
# Builds the arris wasm probe, measures its size, runs it under Node and writes result.json.
set -uo pipefail
cd "$(dirname "$0")"

WASM=target/wasm32-unknown-unknown/release/wasm_arris.wasm
if ! cargo build --release --target wasm32-unknown-unknown 2> build.log; then
  note=$(grep -m1 '^error' build.log | sed 's/"/\\"/g')
  printf '{"backend":"arris","compiles":false,"raw_bytes":null,"gzip_bytes":null,"run":null,"note":"%s"}\n' "$note" > result.json
  cat result.json
  exit 1
fi
wasm-bindgen --target nodejs --out-dir pkg "$WASM"
BG=pkg/wasm_arris_bg.wasm
raw=$(wc -c < "$BG" | tr -d ' ')
gz=$(gzip -9 -c "$BG" | wc -c | tr -d ' ')
out=$(node run.mjs 2> run.log)
run=$(printf '%s' "$out" | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{try{console.log(JSON.parse(s).summary)}catch{console.log("node run failed")}})')
printf '%s' "$out" > run.json
note="sizes are the wasm-bindgen output (release, lto); per-case detail in run.json; Rust-side timings are 0 on wasm (timed() has no clock), JS times in run.json"
printf '{"backend":"arris","compiles":true,"raw_bytes":%s,"gzip_bytes":%s,"run":"%s","note":"%s"}\n' "$raw" "$gz" "$run" "$note" > result.json
cat result.json
