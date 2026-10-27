#!/usr/bin/env bash
# Builds the solidcore wasm probe, records its size, runs it in Node and writes result.json.
set -uo pipefail
cd "$(dirname "$0")"

log=$(mktemp)
if ! cargo build --release --target wasm32-unknown-unknown 2>"$log"; then
  err=$(grep -m1 -A3 '^error' "$log" | tr '\n' ' ' | sed 's/"/\\"/g')
  printf '{"backend":"solidcore","compiles":false,"raw_bytes":null,"gzip_bytes":null,"run":null,"note":"%s"}\n' "$err" > result.json
  cat result.json
  exit 1
fi

wasm=target/wasm32-unknown-unknown/release/wasm_solidcore.wasm
wasm-bindgen --target nodejs --out-dir pkg "$wasm"
raw=$(wc -c < "$wasm" | tr -d ' ')
gz=$(gzip -9 -c "$wasm" | wc -c | tr -d ' ')

out=$(node run.mjs)
status=$?
if [ $status -ne 0 ] || [ -z "$out" ]; then
  run="node exited with status $status"
  details='null'
else
  run=$(printf '%s' "$out" | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>console.log(JSON.parse(s).summary))')
  details=$(printf '%s' "$out" | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>console.log(JSON.stringify(JSON.parse(s).results)))')
fi

note="raw = cdylib after wasm-bindgen input (opt-level s, lto); mesh/STEP not exercised (solidcore has neither for a Brep)"
printf '{"backend":"solidcore","compiles":true,"raw_bytes":%s,"gzip_bytes":%s,"run":"%s","note":"%s","cases":%s}\n' \
  "$raw" "$gz" "$run" "$note" "$details" > result.json
cat result.json
