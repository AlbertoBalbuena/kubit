#!/usr/bin/env bash
# Builds the wasm probe, records sizes, runs it in node and writes result.json.
set -u
cd "$(dirname "$0")"
LOG=$(mktemp)
if ! cargo build --release --target wasm32-unknown-unknown >"$LOG" 2>&1; then
  NOTE=$(grep -m1 -A3 '^error' "$LOG" | tr '\n' ' ' | sed 's/"/\\"/g')
  printf '{"backend":"truck","compiles":false,"raw_bytes":null,"gzip_bytes":null,"run":null,"note":"%s"}\n' "$NOTE" > result.json
  exit 0
fi
WASM=target/wasm32-unknown-unknown/release/wasm_truck.wasm
rm -rf pkg && wasm-bindgen --target nodejs --out-dir pkg "$WASM" >/dev/null 2>&1
RAW=$(wc -c < pkg/wasm_truck_bg.wasm | tr -d ' ')
GZ=$(gzip -9 -c pkg/wasm_truck_bg.wasm | wc -c | tr -d ' ')
OUT=$(node run.mjs 2>run.log)
node -e '
const [raw, gz, out] = [process.argv[1], process.argv[2], process.argv[3]];
let run = null, note = "";
try { const j = JSON.parse(out); run = j.summary;
  note = j.rows.filter(r => r.status !== "ok" && r.status !== "not_exposed").map(r => `${r.id}: ${r.status} ${r.message ?? ""}`).join("; ");
} catch (e) { run = "node run failed"; note = out.slice(0, 200); }
console.log(JSON.stringify({ backend: "truck", compiles: true, raw_bytes: +raw, gzip_bytes: +gz, run, note }, null, 1));
' "$RAW" "$GZ" "$OUT" > result.json
cat result.json
