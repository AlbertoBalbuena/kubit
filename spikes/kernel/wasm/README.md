# wasm32 probes (test 14)

One crate per backend: `wasm/<backend>/`, a `cdylib` with **its own `[workspace]`** (it is
excluded from the spike workspace so its lockfile and target dir never collide).

Contract for `wasm/<backend>/`:
- `Cargo.toml`: `crate-type = ["cdylib"]`, depends on `backend-<backend>` and `harness` by path,
  and `wasm-bindgen` (verify the version against the installed CLI: `wasm-bindgen --version`).
- `.cargo/config.toml`:
  ```toml
  [target.wasm32-unknown-unknown]
  rustflags = ["-C", "link-arg=-zstack-size=8388608"]   # 8 MiB, see solidcore README
  ```
- Export `#[wasm_bindgen] pub fn run_case(id: &str) -> String`: runs `Backend::run(id, 0.01)` and
  returns a JSON summary (status, error message, volume rel. error via `harness::measure::evaluate`).
- `run.mjs`: loads the `wasm-bindgen --target nodejs` output with Node, runs the cases of tests 1–9
  (`1a 1b 2a 2b 3a 3b 4 5a 5b 5c 5d 6 7a 7b 8a 8b 9a 9b`), times each call with `performance.now()`
  and catches traps (a Rust panic in wasm is a `RuntimeError: unreachable`).
- `measure.sh`: `cargo build --release --target wasm32-unknown-unknown`, then `wasm-bindgen`,
  records raw and gzip size of the `.wasm`, runs `node run.mjs`, and writes `result.json`:
  ```json
  {"backend":"arris","compiles":true,"raw_bytes":123,"gzip_bytes":45,
   "run":"14/18 ok, 2 N/E, 2 trap in node","note":"…"}
  ```
  If it does not compile, `compiles:false` and the first compiler error in `note`.
