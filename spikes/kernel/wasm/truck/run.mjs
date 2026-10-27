// Runs the cases of tests 1-9 through the wasm build. Each case runs in its own worker
// thread (fresh wasm instance) with a timeout, so a trap or a hang cannot hide other cases.
import { Worker, isMainThread, parentPort, workerData } from 'node:worker_threads';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const TIMEOUT_MS = 60_000;

if (!isMainThread) {
  const require = createRequire(import.meta.url);
  const wasm = require('./pkg/wasm_truck.js');
  const t0 = performance.now();
  let row;
  try {
    row = JSON.parse(wasm.run_case(workerData.id));
  } catch (e) {
    row = { status: 'trap', message: String(e).split('\n')[0] };
  }
  row.ms = Math.round(performance.now() - t0);
  parentPort.postMessage(row);
} else {
  const ids = '1a 1b 2a 2b 3a 3b 4 5a 5b 5c 5d 6 7a 7b 8a 8b 9a 9b'.split(' ');
  const runOne = (id) =>
    new Promise((resolve) => {
      const w = new Worker(fileURLToPath(import.meta.url), { workerData: { id }, resourceLimits: { stackSizeMb: 8 } });
      const timer = setTimeout(() => {
        w.terminate();
        resolve({ status: 'timeout', message: `killed after ${TIMEOUT_MS / 1000}s`, ms: TIMEOUT_MS });
      }, TIMEOUT_MS);
      w.once('message', (row) => { clearTimeout(timer); resolve(row); });
      w.once('error', (e) => { clearTimeout(timer); resolve({ status: 'trap', message: String(e).split('\n')[0], ms: 0 }); });
    });
  const rows = [];
  for (const id of ids) {
    const row = { id, ...(await runOne(id)) };
    rows.push(row);
    console.error(`${id}: ${row.status} ${row.ms}ms ${row.message ?? ''}`);
  }
  const count = (s) => rows.filter((r) => r.status === s).length;
  const summary = `${count('ok')}/${rows.length} ok, ${count('not_exposed')} N/E, ${count('error')} error, ${count('trap')} trap, ${count('timeout')} timeout in node`;
  console.log(JSON.stringify({ summary, rows }));
}
