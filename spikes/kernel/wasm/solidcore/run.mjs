// Runs the cases of tests 1–9 through the wasm build in Node and prints a JSON summary.
import { createRequire } from "node:module";
import { performance } from "node:perf_hooks";

const require = createRequire(import.meta.url);
const { run_case } = require("./pkg/wasm_solidcore.js");

const CASES = ["1a", "1b", "2a", "2b", "3a", "3b", "4", "5a", "5b", "5c", "5d", "6", "7a", "7b", "8a", "8b", "9a", "9b"];
const results = [];
for (const id of CASES) {
  const t0 = performance.now();
  let r;
  try {
    r = JSON.parse(run_case(id));
  } catch (e) {
    r = { id, status: "trap", message: String(e) };
  }
  r.ms = performance.now() - t0;
  results.push(r);
  console.error(`${id}: ${r.status} ${r.ms.toFixed(1)} ms ${r.message ?? ""}`);
}
const count = (s) => results.filter((r) => r.status === s).length;
const summary = `${count("ok")}/${CASES.length} ok, ${count("not_exposed")} N/E, ${count("failed")} failed, ${count("trap")} trap in node`;
console.log(JSON.stringify({ summary, results }));
