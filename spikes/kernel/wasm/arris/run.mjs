// Runs the tests 1–9 cases of the arris wasm probe under Node and prints a JSON summary.
import { createRequire } from "node:module";
import { performance } from "node:perf_hooks";

const require = createRequire(import.meta.url);
const probe = require("./pkg/wasm_arris.js");

const CASES = ["1a", "1b", "2a", "2b", "3a", "3b", "4", "5a", "5b", "5c", "5d", "6", "7a", "7b", "8a", "8b", "9a", "9b"];
const results = [];
for (const id of CASES) {
  const t0 = performance.now();
  let entry;
  try {
    entry = JSON.parse(probe.run_case(id));
  } catch (e) {
    entry = { id, status: "trap", error: String(e) };
  }
  entry.ms = performance.now() - t0;
  results.push(entry);
  console.error(`${id}: ${entry.status} ${entry.ms.toFixed(1)} ms ${entry.error ?? ""}`);
}
const count = (s) => results.filter((r) => r.status === s).length;
const summary = `${count("ok")}/${CASES.length} ok, ${count("not_exposed")} N/E, ${count("failed")} failed, ${count("trap")} trap in node`;
console.log(JSON.stringify({ summary, results }));
