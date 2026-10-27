// Runs the monstertruck wasm probe in Node: cases of tests 1–9, timed, traps caught.
import { createRequire } from "node:module";
import { performance } from "node:perf_hooks";

const require = createRequire(import.meta.url);
const CASES = ["1a", "1b", "2a", "2b", "3a", "3b", "4", "5a", "5b", "5c", "5d", "6", "7a", "7b", "8a", "8b", "9a", "9b"];

let wasm;
const results = [];
for (const id of CASES) {
  // A trap can leave the instance unusable, so reload the module for every case.
  const path = require.resolve("./pkg/wasm_monstertruck.js");
  delete require.cache[path];
  wasm = require(path);
  const t0 = performance.now();
  let out;
  try {
    out = JSON.parse(wasm.run_case(id));
  } catch (e) {
    out = { id, status: "trap", error: String(e && e.message ? e.message : e) };
  }
  out.ms = Math.round((performance.now() - t0) * 10) / 10;
  results.push(out);
  console.error(JSON.stringify(out));
}
const count = (s) => results.filter((r) => r.status === s).length;
const invalid = results.filter((r) => r.status === "ok" && r.valid === false).length;
const summary = `${count("ok") - invalid}/${CASES.length} ok, ${invalid} invalid, ${count("not_exposed")} N/E, ${count("failed")} failed, ${count("trap")} trap in node`;
console.log(JSON.stringify({ summary, results }));
