//! Renders `results/results.json` (+ `results/wasm.json`) into `results/table.md`.

use crate::{Record, ALL_BACKENDS};
use harness::History;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

#[derive(Debug, Default, Deserialize)]
pub struct WasmResult {
    pub backend: String,
    pub compiles: bool,
    pub raw_bytes: Option<u64>,
    pub gzip_bytes: Option<u64>,
    /// e.g. "9/9 passed in node"
    pub run: Option<String>,
    pub note: Option<String>,
}

const TITLES: [&str; 16] = [
    "Caja − cilindro (pasante, ciego)",
    "Booleana coplanar (cara, arista)",
    "Booleana tangente (cilindro, esfera)",
    "Unión de 100 cilindros",
    "Fillet (arista, cadena, vértice, variable)",
    "Fillet después de booleana",
    "Chaflán (d-d, d-ángulo)",
    "Shell (caja, cilindro con fillet)",
    "Push/pull de cara fileteada",
    "Sweep hélice / loft",
    "Revolve tocando el eje",
    "STEP round-trip + import NIST",
    "Teselado watertight",
    "Build wasm32",
    "Hostiles (proptest) en 1, 5, 9",
    "Historia en 1, 5, 9",
];

/// Grade of one record: pass / wrong / error / ne / panic / crash / timeout / skip.
pub fn grade(r: &Record) -> &'static str {
    match r.status.as_str() {
        "ok" => {
            let Some(e) = &r.eval else { return "pass" };
            if e.valid == Some(false) || e.faces_ok == Some(false) {
                return "wrong";
            }
            let limit = if e.volume_method.as_deref() == Some("native") { 1e-6 } else { 1e-3 };
            let limit_rt = 1e-3;
            if e.volume_rel_err.is_some_and(|x| !(x <= limit)) || r.roundtrip_rel_err.is_some_and(|x| !(x <= limit_rt)) {
                return "wrong";
            }
            "pass"
        }
        "error" => "error",
        "not_exposed" => "ne",
        "panic" => "panic",
        "crash" => "crash",
        "timeout" => "timeout",
        "skipped" => "skip",
        _ => "unavail",
    }
}

fn symbol(g: &str) -> &'static str {
    match g {
        "pass" => "✅",
        "wrong" => "⚠️",
        "error" => "❌",
        "ne" => "N/E",
        "panic" => "💥",
        "crash" => "☠️",
        "timeout" => "⏱",
        "skip" => "↷",
        _ => "?",
    }
}

fn fmt_secs(s: f64) -> String {
    if s < 1e-3 {
        format!("{:.0} µs", s * 1e6)
    } else if s < 1.0 {
        format!("{:.1} ms", s * 1e3)
    } else {
        format!("{s:.2} s")
    }
}

fn fmt_err(x: f64) -> String {
    if x == 0.0 {
        "0".into()
    } else {
        format!("{x:.1e}")
    }
}

fn summary_cell(recs: &[&Record]) -> String {
    if recs.is_empty() {
        return "—".into();
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for r in recs {
        *counts.entry(grade(r)).or_insert(0) += 1;
    }
    let order = ["pass", "wrong", "error", "ne", "panic", "crash", "timeout", "skip", "unavail"];
    let mut s = order
        .iter()
        .filter_map(|g| counts.get(g).map(|n| format!("{}{}", symbol(g), n)))
        .collect::<Vec<_>>()
        .join(" ");
    let worst = recs
        .iter()
        .filter(|r| r.status == "ok")
        .filter_map(|r| r.eval.as_ref().and_then(|e| e.volume_rel_err))
        .fold(None, |acc: Option<f64>, x| Some(acc.map_or(x, |a| a.max(x))));
    if let Some(w) = worst {
        let _ = write!(s, " · εV≤{}", fmt_err(w));
    }
    let mut times: Vec<f64> = recs.iter().filter(|r| r.status == "ok").filter_map(|r| r.op_seconds).collect();
    times.sort_by(f64::total_cmp);
    if let Some(t) = times.get(times.len() / 2) {
        let _ = write!(s, " · {}", fmt_secs(*t));
    }
    s
}

fn history_cell(recs: &[&Record]) -> String {
    let parts: Vec<String> = recs
        .iter()
        .map(|r| {
            let h = match &r.history {
                Some(History::Native { generated, modified, deleted, .. }) => format!("nativa g{generated}/m{modified}/d{deleted}"),
                Some(History::Partial(s)) => format!("parcial ({s})"),
                Some(History::None) => "no".into(),
                None => format!("{} (sin resultado)", symbol(grade(r))),
            };
            format!("{}: {h}", r.case)
        })
        .collect();
    if parts.is_empty() {
        "—".into()
    } else {
        parts.join("; ")
    }
}

fn hostile_cell(recs: &[&Record]) -> String {
    let mut out = Vec::new();
    for r in recs {
        let fam = r.case.trim_start_matches("15:");
        match (&r.hostile, r.status.as_str()) {
            (Some(h), _) if h.not_exposed == h.total => out.push(format!("{fam}: N/E")),
            (Some(h), _) => out.push(format!("{fam}: {}💥/{}", h.panics, h.total)),
            (None, s) => out.push(format!("{fam}: {}", symbol(grade(r)).replace('?', s))),
        }
    }
    out.join(" · ")
}

pub fn write(dir: &Path, records: &[Record]) -> ExitCode {
    let wasm: Vec<WasmResult> = std::fs::read_to_string(dir.join("wasm.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let mut md = String::new();
    let _ = writeln!(md, "# Resultados del spike de kernels\n");
    let _ = writeln!(md, "Generado por `spike report` a partir de `results.json`. Leyenda: ✅ correcto · ⚠️ terminó pero el resultado es inválido o el volumen se sale de tolerancia · ❌ error limpio · N/E operación no expuesta · 💥 panic · ☠️ crash del proceso · ⏱ timeout · ↷ omitido (el caso fuente falló). εV = peor error relativo de volumen; tiempo = mediana de los casos correctos.\n");
    let _ = writeln!(md, "## Resumen: 16 pruebas × backends\n");
    let _ = writeln!(md, "| # | Prueba | {} |", ALL_BACKENDS.join(" | "));
    let _ = writeln!(md, "|---|---|{}", "---|".repeat(ALL_BACKENDS.len()));
    for test in 1..=16u8 {
        let title = TITLES.get(usize::from(test) - 1).copied().unwrap_or("");
        let cells: Vec<String> = ALL_BACKENDS
            .iter()
            .map(|b| {
                let of = |pred: &dyn Fn(&Record) -> bool| records.iter().filter(|r| r.backend == *b && pred(r)).collect::<Vec<_>>();
                match test {
                    12 => {
                        let rt = of(&|r| r.case.starts_with("12rt:"));
                        let imp = of(&|r| r.case.starts_with("12imp:"));
                        format!("RT {} / import {}", summary_cell(&rt), summary_cell(&imp))
                    }
                    14 => wasm.iter().find(|w| w.backend == *b).map_or("—".into(), |w| {
                        if w.compiles {
                            format!("✅ {} KiB ({} KiB gz) · {}", w.raw_bytes.unwrap_or(0) / 1024, w.gzip_bytes.unwrap_or(0) / 1024, w.run.as_deref().unwrap_or("sin ejecutar"))
                        } else {
                            format!("❌ {}", w.note.as_deref().unwrap_or("no compila"))
                        }
                    }),
                    15 => hostile_cell(&of(&|r| r.case.starts_with("15:"))),
                    16 => history_cell(&of(&|r| ["1a", "5a", "9a"].contains(&r.case.as_str()))),
                    t => summary_cell(&of(&|r| r.test == t && !r.case.contains(':'))),
                }
            })
            .collect();
        let _ = writeln!(md, "| {test} | {title} | {} |", cells.join(" | "));
    }

    let _ = writeln!(md, "\n## Detalle por sub-caso\n");
    let _ = writeln!(md, "| Caso | Backend | Resultado | Tiempo | Volumen (método) | εV | εA | Válido | V/E/F/L/S | Mensaje |");
    let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|---|");
    for r in records {
        if r.case.starts_with("15:") {
            continue;
        }
        let e = r.eval.as_ref();
        let vol = e.and_then(|e| e.volume.map(|v| format!("{v:.3} ({})", e.volume_method.as_deref().unwrap_or("?")))).unwrap_or_default();
        let ev = e.and_then(|e| e.volume_rel_err).map(fmt_err).unwrap_or_default();
        let ea = e.and_then(|e| e.area_rel_err).map(fmt_err).unwrap_or_default();
        let rt = r.roundtrip_rel_err.map(|x| format!(" (RT {})", fmt_err(x))).unwrap_or_default();
        let valid = e
            .map(|e| {
                let m = e.mesh.as_ref().map(|m| format!(" b{} nm{} mo{}", m.boundary_edges, m.nonmanifold_edges, m.misoriented_edges)).unwrap_or_default();
                let d = e.mesh.as_ref().and_then(|m| m.max_sphere_deviation).map(|d| format!(" dev{d:.4}")).unwrap_or_default();
                let eu = e.euler_ok.map(|x| if x { " χ✓" } else { " χ✗" }).unwrap_or("");
                format!("{}{m}{d}{eu}", e.valid.map_or("?", |v| if v { "sí" } else { "no" }))
            })
            .unwrap_or_default();
        let topo = r.topo.as_ref().map(|t| format!("{}/{}/{}/{}/{}", t.vertices, t.edges, t.faces, t.loops.map_or("?".into(), |l| l.to_string()), t.shells)).unwrap_or_default();
        let msg: String = r.message.as_deref().unwrap_or("").replace('|', "\\|").replace('\n', " ").chars().take(140).collect();
        let case = r.case.rsplit('/').next().unwrap_or(&r.case);
        let _ = writeln!(
            md,
            "| {case} | {} | {} | {} | {vol} | {ev}{rt} | {ea} | {valid} | {topo} | {msg} |",
            r.backend,
            symbol(grade(r)),
            r.op_seconds.map(fmt_secs).unwrap_or_default()
        );
    }

    let _ = writeln!(md, "\n## Hostiles (prueba 15)\n");
    let _ = writeln!(md, "| Familia | Backend | Total | OK | Error limpio | N/E | Panics | Primer panic |");
    let _ = writeln!(md, "|---|---|---|---|---|---|---|---|");
    for r in records.iter().filter(|r| r.case.starts_with("15:")) {
        match &r.hostile {
            Some(h) => {
                let first: String = h.first_panic.as_deref().unwrap_or("").replace('|', "\\|").chars().take(160).collect();
                let _ = writeln!(md, "| {} | {} | {} | {} | {} | {} | {} | {first} |", r.case, r.backend, h.total, h.ok, h.clean_error, h.not_exposed, h.panics);
            }
            None => {
                let msg: String = r.message.as_deref().unwrap_or("").replace('|', "\\|").chars().take(160).collect();
                let _ = writeln!(md, "| {} | {} | — | — | — | — | {} | {msg} |", r.case, r.backend, symbol(grade(r)));
            }
        }
    }
    match std::fs::write(dir.join("table.md"), md) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("write table.md: {e}");
            ExitCode::FAILURE
        }
    }
}
