#!/usr/bin/env bash
# Builds the monstertruck wasm probe, measures its size, runs it in Node and writes result.json.
set -uo pipefail
cd "$(dirname "$0")"

if ! build_log=$(cargo build --release --target wasm32-unknown-unknown 2>&1); then
  err=$(printf '%s\n' "$build_log" | grep -m1 '^error' || echo "build failed")
  jq -n --arg note "$err" '{backend:"monstertruck",compiles:false,raw_bytes:null,gzip_bytes:null,run:null,note:$note}' > result.json
  exit 1
fi

wasm=target/wasm32-unknown-unknown/release/wasm_monstertruck.wasm
wasm-bindgen --target nodejs --out-dir pkg "$wasm"
raw=$(wc -c < "$wasm" | tr -d ' ')
gz=$(gzip -9 -c "$wasm" | wc -c | tr -d ' ')

run_json=$(node run.mjs 2> run.log)
summary=$(printf '%s' "$run_json" | jq -r .summary)
jq -n --argjson raw "$raw" --argjson gz "$gz" --arg run "$summary" --argjson results "$(printf '%s' "$run_json" | jq .results)" \
  '{backend:"monstertruck",compiles:true,raw_bytes:$raw,gzip_bytes:$gz,run:$run,
    note:"sizes are the cargo wasm (pre wasm-bindgen, no wasm-opt); times in results[].ms from Node performance.now()",
    results:$results}' > result.json
cat result.json | jq -c '{raw_bytes,gzip_bytes,run}'
