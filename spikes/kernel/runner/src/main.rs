//! `spike` — runs the kernel corpus. Every (backend, case) pair runs in its own
//! child process so an abort or stack overflow in one backend cannot hide or
//! contaminate other results.
//!
//! spike run --backend <name> --case <id>     one case, prints one JSON record
//! spike all [--backends a,b] [--only <prefix>] [--timeout <s>]
//! spike report                                regenerate results/table.md from results/results.json

mod report;

use harness::corpus::{self, Oracle};
use harness::measure::{evaluate, Evaluation};
use harness::{hostile, Backend, History, Produced, SpikeError, Topo};
use serde::{Deserialize, Serialize};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Chord tolerance for the meshes used to measure volume when the backend has no
/// mass properties. Case 13 overrides it.
const MEASURE_TOL: f64 = 0.002;
const REPEATS: usize = 5;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HostileCounts {
    pub total: usize,
    pub ok: usize,
    pub clean_error: usize,
    pub not_exposed: usize,
    pub panics: usize,
    pub first_panic: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Record {
    pub backend: String,
    pub case: String,
    pub test: u8,
    /// ok | error | not_exposed | panic | crash | timeout | unavailable
    pub status: String,
    pub message: Option<String>,
    pub op_seconds: Option<f64>,
    pub runs: usize,
    pub eval: Option<Evaluation>,
    pub topo: Option<Topo>,
    pub history: Option<History>,
    pub step_written: Option<bool>,
    pub step_error: Option<String>,
    /// Case 12 round-trip: |V_reimported − V_original| / V_original.
    pub roundtrip_rel_err: Option<f64>,
    pub hostile: Option<HostileCounts>,
    pub notes: Vec<String>,
    pub wall_seconds: Option<f64>,
}

fn backends() -> Vec<Box<dyn Backend>> {
    #[allow(unused_mut)]
    let mut v: Vec<Box<dyn Backend>> = Vec::new();
    #[cfg(feature = "monstertruck")]
    v.push(Box::new(backend_monstertruck::Monstertruck));
    #[cfg(feature = "truck")]
    v.push(Box::new(backend_truck::Truck));
    #[cfg(feature = "solidcore")]
    v.push(Box::new(backend_solidcore::Solidcore));
    #[cfg(feature = "arris")]
    v.push(Box::new(backend_arris::Arris));
    v
}

pub const ALL_BACKENDS: [&str; 4] = ["monstertruck", "truck", "solidcore", "arris"];

static PANIC_MSG: Mutex<Option<String>> = Mutex::new(None);

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let loc = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic>".to_owned());
        if let Ok(mut slot) = PANIC_MSG.lock() {
            *slot = Some(format!("{msg}{loc}"));
        }
    }));
}

fn take_panic() -> String {
    PANIC_MSG.lock().ok().and_then(|mut s| s.take()).unwrap_or_else(|| "panic".to_owned())
}

enum Outcome<T> {
    Ok(T),
    Err(SpikeError),
    Panic(String),
}

fn guarded<T>(f: impl FnOnce() -> Result<T, SpikeError>) -> Outcome<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(v)) => Outcome::Ok(v),
        Ok(Err(e)) => Outcome::Err(e),
        Err(_) => Outcome::Panic(take_panic()),
    }
}

fn fill_error(rec: &mut Record, e: SpikeError) {
    match e {
        SpikeError::NotExposed(m) => {
            rec.status = "not_exposed".into();
            rec.message = Some(m);
        }
        SpikeError::Failed(m) => {
            rec.status = "error".into();
            rec.message = Some(m);
        }
    }
}

fn fill_produced(rec: &mut Record, p: &Produced, oracle: &Oracle) {
    rec.status = "ok".into();
    rec.eval = Some(evaluate(p.mesh.as_ref(), p.topo.as_ref(), p.native_volume, p.native_area, oracle));
    rec.topo = p.topo.clone();
    rec.history = p.history.clone();
    rec.step_written = Some(p.step.is_some());
    rec.step_error = p.step_error.clone();
    rec.notes = p.notes.clone();
}

fn median(mut xs: Vec<f64>) -> Option<f64> {
    xs.sort_by(f64::total_cmp);
    xs.get(xs.len() / 2).copied()
}

fn test_number(case: &str) -> u8 {
    if case.starts_with("12") {
        12
    } else if let Some(rest) = case.strip_prefix("15:") {
        let _ = rest;
        15
    } else {
        corpus::find(case).map_or(0, |c| c.test)
    }
}

/// Runs one case in-process. Called inside the child process.
fn run_one(backend: &dyn Backend, case: &str) -> Record {
    let mut rec = Record { backend: backend.name().into(), case: case.into(), test: test_number(case), ..Record::default() };

    if let Some(test) = case.strip_prefix("15:") {
        let Some(family) = test.parse::<u8>().ok().and_then(hostile::family_for_test) else {
            rec.status = "error".into();
            rec.message = Some(format!("unknown hostile family {test}"));
            return rec;
        };
        let sets = hostile::parameter_sets(family);
        let mut counts = HostileCounts { total: sets.len(), ..HostileCounts::default() };
        for (i, params) in sets.iter().enumerate() {
            // Breadcrumb for the parent if the process dies on this input.
            eprintln!("HOSTILE {i} {params:?}");
            match guarded(|| backend.hostile(family, params)) {
                Outcome::Ok(()) => counts.ok += 1,
                Outcome::Err(SpikeError::NotExposed(_)) => counts.not_exposed += 1,
                Outcome::Err(SpikeError::Failed(_)) => counts.clean_error += 1,
                Outcome::Panic(m) => {
                    counts.panics += 1;
                    if counts.first_panic.is_none() {
                        counts.first_panic = Some(format!("{params:?}: {m}"));
                    }
                }
            }
        }
        rec.status = if counts.panics > 0 {
            "panic".into()
        } else if counts.not_exposed == counts.total {
            "not_exposed".into()
        } else {
            "ok".into()
        };
        rec.hostile = Some(counts);
        return rec;
    }

    if let Some(src) = case.strip_prefix("12rt:") {
        let Some(c) = corpus::find(src) else {
            rec.status = "error".into();
            rec.message = Some(format!("unknown case {src}"));
            return rec;
        };
        let original = match guarded(|| backend.run(src, MEASURE_TOL)) {
            Outcome::Ok(p) => p,
            Outcome::Err(e) => {
                rec.status = "skipped".into();
                rec.message = Some(format!("source case did not produce a solid: {e}"));
                return rec;
            }
            Outcome::Panic(m) => {
                rec.status = "skipped".into();
                rec.message = Some(format!("source case panicked: {m}"));
                return rec;
            }
        };
        let original_eval = evaluate(original.mesh.as_ref(), original.topo.as_ref(), original.native_volume, original.native_area, &c.oracle);
        let Some(text) = original.step.clone() else {
            fill_error(&mut rec, SpikeError::NotExposed(original.step_error.unwrap_or_else(|| "STEP export".into())));
            return rec;
        };
        match guarded(|| backend.import_step(&text, MEASURE_TOL)) {
            Outcome::Ok(p) => {
                fill_produced(&mut rec, &p, &c.oracle);
                rec.roundtrip_rel_err = rec
                    .eval
                    .as_ref()
                    .and_then(|e| e.volume)
                    .zip(original_eval.volume)
                    .map(|(v, o)| (v - o).abs() / o.abs().max(f64::MIN_POSITIVE));
            }
            Outcome::Err(e) => fill_error(&mut rec, e),
            Outcome::Panic(m) => {
                rec.status = "panic".into();
                rec.message = Some(m);
            }
        }
        return rec;
    }

    if let Some(path) = case.strip_prefix("12imp:") {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                rec.status = "error".into();
                rec.message = Some(format!("read {path}: {e}"));
                return rec;
            }
        };
        let (out, secs) = harness::timed(|| guarded(|| backend.import_step(&text, 0.05)));
        rec.op_seconds = Some(secs);
        rec.runs = 1;
        match out {
            Outcome::Ok(p) => fill_produced(&mut rec, &p, &Oracle { genus: None, ..Oracle::default() }),
            Outcome::Err(e) => fill_error(&mut rec, e),
            Outcome::Panic(m) => {
                rec.status = "panic".into();
                rec.message = Some(m);
            }
        }
        return rec;
    }

    let Some(c) = corpus::find(case) else {
        rec.status = "error".into();
        rec.message = Some(format!("unknown case {case}"));
        return rec;
    };
    let tol = if c.test == 13 { corpus::tessellation_tol(case) } else { MEASURE_TOL };
    let mut times = Vec::new();
    // Repeats are for timing noise only; stop early when one pass (operation plus
    // tessellation and measurement) is already slow, so costly cases fit the timeout.
    let started = std::time::Instant::now();
    for i in 0..REPEATS {
        if i > 0 && started.elapsed().as_secs_f64() > 10.0 {
            break;
        }
        match guarded(|| backend.run(case, tol)) {
            Outcome::Ok(p) => {
                times.push(p.op_seconds);
                if i == 0 {
                    fill_produced(&mut rec, &p, &c.oracle);
                }
                if p.op_seconds > 10.0 {
                    break;
                }
            }
            Outcome::Err(e) => {
                fill_error(&mut rec, e);
                break;
            }
            Outcome::Panic(m) => {
                rec.status = "panic".into();
                rec.message = Some(m);
                break;
            }
        }
    }
    rec.runs = times.len();
    rec.op_seconds = median(times);
    rec
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn results_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../results")
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/step")
}

fn step_fixtures() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for sub in ["nist", "local"] {
        if let Ok(rd) = std::fs::read_dir(fixtures_dir().join(sub)) {
            let mut files: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("stp") || x.eq_ignore_ascii_case("step")))
                .collect();
            files.sort();
            out.extend(files);
        }
    }
    out
}

fn all_case_ids() -> Vec<String> {
    let mut ids: Vec<String> = corpus::cases().iter().map(|c| c.id.to_owned()).collect();
    ids.extend(corpus::cases().iter().filter(|c| c.step_roundtrip).map(|c| format!("12rt:{}", c.id)));
    ids.extend(step_fixtures().iter().map(|p| format!("12imp:{}", p.display())));
    ids.extend(["15:1", "15:5", "15:9"].map(String::from));
    ids
}

/// Spawns `spike run` for one pair, with a timeout.
fn spawn_one(exe: &Path, backend: &str, case: &str, timeout: Duration) -> Record {
    let mut rec = Record { backend: backend.into(), case: case.into(), test: test_number(case), ..Record::default() };
    let tmp = std::env::temp_dir().join(format!("spike-{}-{}.stderr", backend, std::process::id()));
    let stderr = std::fs::File::create(&tmp).map(Stdio::from).unwrap_or_else(|_| Stdio::null());
    let start = Instant::now();
    let child = Command::new(exe)
        .args(["run", "--backend", backend, "--case", case])
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            rec.status = "crash".into();
            rec.message = Some(format!("spawn: {e}"));
            return rec;
        }
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => break None,
        }
    };
    let wall = start.elapsed().as_secs_f64();
    let mut stdout = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = std::io::Read::read_to_string(&mut out, &mut stdout);
    }
    let last_stderr = std::fs::read_to_string(&tmp)
        .ok()
        .and_then(|s| s.lines().rev().find(|l| !l.trim().is_empty()).map(str::to_owned));
    let _ = std::fs::remove_file(&tmp);
    match status {
        None => {
            rec.status = "timeout".into();
            rec.message = Some(format!("killed after {}s; last stderr: {}", timeout.as_secs(), last_stderr.unwrap_or_default()));
        }
        Some(s) if s.success() => match stdout.lines().last().map(serde_json::from_str::<Record>) {
            Some(Ok(r)) => rec = r,
            _ => {
                rec.status = "crash".into();
                rec.message = Some("child produced no record".into());
            }
        },
        Some(s) => {
            rec.status = "crash".into();
            #[cfg(unix)]
            let sig = std::os::unix::process::ExitStatusExt::signal(&s).map(|n| format!(" (signal {n})")).unwrap_or_default();
            #[cfg(not(unix))]
            let sig = String::new();
            rec.message = Some(format!("exit {s}{sig}; last stderr: {}", last_stderr.unwrap_or_default()));
        }
    }
    rec.wall_seconds = Some(wall);
    rec
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("run") => {
            install_panic_hook();
            let (Some(b), Some(case)) = (arg(&args, "--backend"), arg(&args, "--case")) else {
                eprintln!("usage: spike run --backend <name> --case <id>");
                return ExitCode::FAILURE;
            };
            let list = backends();
            let rec = match list.iter().find(|x| x.name() == b) {
                Some(backend) => run_one(backend.as_ref(), &case),
                None => Record { backend: b, case: case.clone(), test: test_number(&case), status: "unavailable".into(), message: Some("backend not compiled into this runner".into()), ..Record::default() },
            };
            match serde_json::to_string(&rec) {
                Ok(s) => {
                    println!("{s}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("serialize: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("all") => {
            let Ok(exe) = std::env::current_exe() else { return ExitCode::FAILURE };
            let compiled: Vec<&str> = backends().iter().map(|b| b.name()).collect();
            let wanted: Vec<String> = arg(&args, "--backends").map_or_else(|| ALL_BACKENDS.map(String::from).to_vec(), |s| s.split(',').map(String::from).collect());
            let only = arg(&args, "--only");
            let timeout = Duration::from_secs(arg(&args, "--timeout").and_then(|s| s.parse().ok()).unwrap_or(120));
            let path = results_dir().join("results.json");
            // Merge with previous results so partial reruns keep the rest.
            let mut records: Vec<Record> = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
            for b in &wanted {
                for case in all_case_ids() {
                    if only.as_ref().is_some_and(|p| !case.starts_with(p.as_str())) {
                        continue;
                    }
                    let rec = if compiled.contains(&b.as_str()) {
                        spawn_one(&exe, b, &case, timeout)
                    } else {
                        Record { backend: b.clone(), case: case.clone(), test: test_number(&case), status: "unavailable".into(), message: Some("backend not compiled into this runner".into()), ..Record::default() }
                    };
                    eprintln!("{:<13} {:<40} {:<12} {}", rec.backend, rec.case, rec.status, rec.message.as_deref().unwrap_or("").chars().take(100).collect::<String>());
                    records.retain(|r| !(r.backend == rec.backend && r.case == rec.case));
                    records.push(rec);
                }
            }
            records.sort_by(|a, b| (a.backend.clone(), a.test, a.case.clone()).cmp(&(b.backend.clone(), b.test, b.case.clone())));
            let _ = std::fs::create_dir_all(results_dir());
            match serde_json::to_string_pretty(&records) {
                Ok(s) => {
                    if let Err(e) = std::fs::write(&path, s) {
                        eprintln!("write {}: {e}", path.display());
                        return ExitCode::FAILURE;
                    }
                }
                Err(e) => {
                    eprintln!("serialize: {e}");
                    return ExitCode::FAILURE;
                }
            }
            report::write(&results_dir(), &records)
        }
        Some("report") => {
            let path = results_dir().join("results.json");
            let records: Vec<Record> = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
            report::write(&results_dir(), &records)
        }
        _ => {
            eprintln!("usage: spike run|all|report");
            ExitCode::FAILURE
        }
    }
}
