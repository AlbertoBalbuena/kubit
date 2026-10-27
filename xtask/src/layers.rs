//! `cargo xtask layers`: checks the workspace dependency graph against `layers.toml`.
//!
//! The check works on a small [`Graph`] built from `cargo metadata`, so the rules can be
//! unit-tested with hand-written graphs.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Deserialize;

/// First layer allowed to use UI crates (and GPU crates without being in `gpu_allowed`).
pub const UI_LAYER: u8 = 5;

/// Name pattern of the kernel backends: the only crates that may use `kernel_libs`.
pub const KERNEL_PATTERN: &str = "kubit-kernel-*";

/// Workspace crates that are build tooling, not product layers.
pub const TOOLING: &[&str] = &["xtask"];

/// `layers.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerMap {
    /// Crates that only some layers may pull.
    pub forbidden: Forbidden,
    /// Layer rule per crate name.
    pub crates: BTreeMap<String, CrateRule>,
    /// Layer rule per name pattern (`*` matches any run of characters).
    #[serde(default)]
    pub patterns: BTreeMap<String, CrateRule>,
}

/// `[forbidden]` in `layers.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Forbidden {
    /// Nothing below [`UI_LAYER`] may pull these.
    #[serde(default)]
    pub ui_only: Vec<String>,
    /// Only `gpu_allowed` crates and layers >= [`UI_LAYER`] may pull these.
    #[serde(default)]
    pub gpu_only: Vec<String>,
    /// Crates below [`UI_LAYER`] that may pull `gpu_only` crates.
    #[serde(default)]
    pub gpu_allowed: Vec<String>,
    /// Only crates matching [`KERNEL_PATTERN`] may pull these.
    #[serde(default)]
    pub kernel_libs: Vec<String>,
}

/// Rule for one crate or pattern.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrateRule {
    /// Layer number; a crate depends only on lower layers.
    pub layer: u8,
    /// Must build for `wasm32-unknown-unknown`.
    #[serde(default)]
    pub wasm: bool,
    /// Crates of the same layer this crate may depend on.
    #[serde(default)]
    pub same_layer: Vec<String>,
}

/// A rule found for a crate, and whether it came from [`KERNEL_PATTERN`].
#[derive(Debug, Clone, Copy)]
pub struct Resolved<'a> {
    /// The rule.
    pub rule: &'a CrateRule,
    /// The crate is a kernel backend (matched [`KERNEL_PATTERN`], not an exact entry).
    pub kernel_backend: bool,
}

impl LayerMap {
    /// Parses `layers.toml` text.
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| format!("layers.toml: {e}"))
    }

    /// The rule for `name`: an exact `[crates]` entry wins over a pattern.
    pub fn resolve(&self, name: &str) -> Option<Resolved<'_>> {
        if let Some(rule) = self.crates.get(name) {
            return Some(Resolved {
                rule,
                kernel_backend: false,
            });
        }
        self.patterns
            .iter()
            .find(|(pattern, _)| glob_match(pattern, name))
            .map(|(pattern, rule)| Resolved {
                rule,
                kernel_backend: pattern == KERNEL_PATTERN,
            })
    }
}

/// Matches `name` against `pattern`, where `*` matches any run of characters.
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let mut parts = pattern.split('*');
    let Some(first) = parts.next() else {
        return name.is_empty();
    };
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let tail: Vec<&str> = parts.collect();
    let Some((last, middle)) = tail.split_last() else {
        // No `*` at all: exact match.
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = rest.get(at + part.len()..).unwrap_or_default(),
            None => return false,
        }
    }
    rest.ends_with(last)
}

/// Dependency graph: normal and build edges only (dev-dependencies do not count).
#[derive(Debug, Default)]
pub struct Graph {
    /// Packages; edges are indices into this list.
    pub nodes: Vec<Node>,
}

/// One package of the graph.
#[derive(Debug)]
pub struct Node {
    /// Package name.
    pub name: String,
    /// Member of the workspace.
    pub workspace: bool,
    /// Normal and build dependencies.
    pub deps: Vec<usize>,
}

impl Graph {
    /// Builds the graph from `cargo metadata` output.
    pub fn from_metadata(metadata: &cargo_metadata::Metadata) -> Result<Self, String> {
        use cargo_metadata::DependencyKind;

        let resolve = metadata
            .resolve
            .as_ref()
            .ok_or("cargo metadata returned no dependency resolution")?;
        let index: BTreeMap<&cargo_metadata::PackageId, usize> = metadata
            .packages
            .iter()
            .enumerate()
            .map(|(i, p)| (&p.id, i))
            .collect();
        let members: BTreeSet<&cargo_metadata::PackageId> =
            metadata.workspace_members.iter().collect();
        let mut nodes: Vec<Node> = metadata
            .packages
            .iter()
            .map(|p| Node {
                name: p.name.to_string(),
                workspace: members.contains(&p.id),
                deps: Vec::new(),
            })
            .collect();
        for node in &resolve.nodes {
            let Some(&from) = index.get(&node.id) else {
                continue;
            };
            let deps: Vec<usize> = node
                .deps
                .iter()
                .filter(|d| {
                    d.dep_kinds
                        .iter()
                        .any(|k| !matches!(k.kind, DependencyKind::Development))
                })
                .filter_map(|d| index.get(&d.pkg).copied())
                .collect();
            if let Some(n) = nodes.get_mut(from) {
                n.deps = deps;
            }
        }
        Ok(Self { nodes })
    }

    /// Adds a node and returns its index.
    #[cfg(test)]
    pub fn add(&mut self, name: &str, workspace: bool) -> usize {
        self.nodes.push(Node {
            name: name.to_owned(),
            workspace,
            deps: Vec::new(),
        });
        self.nodes.len() - 1
    }

    /// Adds an edge `from -> to`.
    #[cfg(test)]
    pub fn depend(&mut self, from: usize, to: usize) {
        if let Some(n) = self.nodes.get_mut(from) {
            n.deps.push(to);
        }
    }

    fn name(&self, i: usize) -> &str {
        self.nodes.get(i).map_or("?", |n| n.name.as_str())
    }

    /// Every package reachable from `root` with the shortest path to it, without walking
    /// through nodes for which `stop` is true (those nodes are still reported).
    fn reach(&self, root: usize, stop: impl Fn(usize) -> bool) -> Vec<(usize, Vec<usize>)> {
        let mut parent: BTreeMap<usize, usize> = BTreeMap::new();
        let mut order = Vec::new();
        let mut queue = VecDeque::from([root]);
        while let Some(at) = queue.pop_front() {
            if at != root && stop(at) {
                continue;
            }
            let deps = self.nodes.get(at).map(|n| n.deps.as_slice()).unwrap_or(&[]);
            for &dep in deps {
                if dep != root && !parent.contains_key(&dep) {
                    parent.insert(dep, at);
                    order.push(dep);
                    queue.push_back(dep);
                }
            }
        }
        order
            .into_iter()
            .map(|target| {
                let mut path = vec![target];
                let mut at = target;
                while let Some(&p) = parent.get(&at) {
                    path.push(p);
                    at = p;
                }
                path.reverse();
                (target, path)
            })
            .collect()
    }

    fn path_string(&self, path: &[usize]) -> String {
        path.iter()
            .map(|&i| self.name(i))
            .collect::<Vec<_>>()
            .join(" -> ")
    }
}

/// Result of [`check`].
#[derive(Debug, Default)]
pub struct Report {
    /// Rule violations; the check fails if this is not empty.
    pub violations: Vec<String>,
    /// Non-fatal findings (for example map entries with no crate).
    pub warnings: Vec<String>,
    /// Workspace crates checked.
    pub checked: usize,
}

/// Checks `graph` against `map`.
pub fn check(map: &LayerMap, graph: &Graph) -> Report {
    let mut report = Report::default();
    let f = &map.forbidden;
    let mut members: Vec<usize> = (0..graph.nodes.len())
        .filter(|&i| graph.nodes.get(i).is_some_and(|n| n.workspace))
        .filter(|&i| !TOOLING.contains(&graph.name(i)))
        .collect();
    members.sort_by(|&a, &b| graph.name(a).cmp(graph.name(b)));

    for &root in &members {
        let name = graph.name(root);
        let Some(me) = map.resolve(name) else {
            report.violations.push(format!(
                "{name}: not in layers.toml (add it under [crates] or match a [patterns] entry)"
            ));
            continue;
        };
        report.checked += 1;
        let layer = me.rule.layer;

        // Direct workspace dependencies: strictly lower layer, or a same_layer exception.
        let deps = graph
            .nodes
            .get(root)
            .map(|n| n.deps.as_slice())
            .unwrap_or(&[]);
        for &dep in deps {
            let dep_node = graph.nodes.get(dep);
            if !dep_node.is_some_and(|n| n.workspace) {
                continue;
            }
            let dep_name = graph.name(dep);
            let Some(other) = map.resolve(dep_name) else {
                continue; // Reported when the dependency itself is checked.
            };
            let dl = other.rule.layer;
            let allowed =
                dl < layer || (dl == layer && me.rule.same_layer.iter().any(|s| s == dep_name));
            if !allowed {
                report.violations.push(format!(
                    "{name} (L{layer}) depends on {dep_name} (L{dl}): a crate may only depend on \
                     lower layers{}",
                    if dl == layer {
                        " (or list it in `same_layer`)"
                    } else {
                        ""
                    }
                ));
            }
        }

        // Forbidden crates, transitively.
        if layer < UI_LAYER {
            let gpu_ok = f.gpu_allowed.iter().any(|g| g == name);
            let mut seen = BTreeSet::new();
            for (target, path) in graph.reach(root, |_| false) {
                let t = graph.name(target);
                let what = if f.ui_only.iter().any(|u| u == t) {
                    Some(format!("UI crate `{t}` (allowed only from L{UI_LAYER})"))
                } else if !gpu_ok && f.gpu_only.iter().any(|g| g == t) {
                    Some(format!(
                        "GPU crate `{t}` (allowed only in {} and from L{UI_LAYER})",
                        f.gpu_allowed.join(", ")
                    ))
                } else {
                    None
                };
                if let Some(what) = what
                    && seen.insert(t.to_owned())
                {
                    report.violations.push(format!(
                        "{name} (L{layer}) pulls {what}: {}",
                        graph.path_string(&path)
                    ));
                }
            }
        }
        if !me.kernel_backend {
            // Kernel backends wrap the kernel library, so the walk stops at them.
            let is_backend = |i: usize| {
                graph.nodes.get(i).is_some_and(|n| n.workspace)
                    && map.resolve(graph.name(i)).is_some_and(|r| r.kernel_backend)
            };
            let mut seen = BTreeSet::new();
            for (target, path) in graph.reach(root, is_backend) {
                let t = graph.name(target);
                if f.kernel_libs.iter().any(|k| k == t) && seen.insert(t.to_owned()) {
                    report.violations.push(format!(
                        "{name} pulls kernel library `{t}` outside a {KERNEL_PATTERN} backend: {}",
                        graph.path_string(&path)
                    ));
                }
            }
        }
    }

    let names: BTreeSet<&str> = members.iter().map(|&i| graph.name(i)).collect();
    for entry in map.crates.keys() {
        if !names.contains(entry.as_str()) {
            report.warnings.push(format!(
                "layers.toml lists `{entry}`, which is not in the workspace"
            ));
        }
    }
    report
}

/// Workspace crates whose rule has `wasm = true`, sorted by name.
pub fn wasm_crates(map: &LayerMap, graph: &Graph) -> Vec<String> {
    let mut out: Vec<String> = graph
        .nodes
        .iter()
        .filter(|n| n.workspace)
        .filter(|n| map.resolve(&n.name).is_some_and(|r| r.rule.wasm))
        .map(|n| n.name.clone())
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = r#"
[forbidden]
ui_only = ["egui", "winit"]
gpu_only = ["wgpu"]
gpu_allowed = ["kubit-render"]
kernel_libs = ["monstertruck"]

[crates]
kubit-math       = { layer = 0, wasm = true }
kubit-kernel-api = { layer = 1, wasm = true }
kubit-mesh       = { layer = 1, wasm = true }
kubit-render     = { layer = 1, wasm = true }
kubit-doc        = { layer = 2, wasm = true }
kubit-engine     = { layer = 4 }
kubit-ui         = { layer = 5 }
kubit            = { layer = 6 }

[patterns."kubit-kernel-*"]
layer = 1
wasm = true
same_layer = ["kubit-kernel-api"]
"#;

    fn map() -> LayerMap {
        LayerMap::parse(MAP).unwrap()
    }

    /// A valid skeleton: math <- kernel-api <- kernel-mt (monstertruck) <- doc <- engine
    /// <- ui (egui, wgpu) <- kubit; render uses wgpu.
    struct Fixture {
        g: Graph,
        math: usize,
        api: usize,
        mt: usize,
        mesh: usize,
        render: usize,
        doc: usize,
        engine: usize,
        egui: usize,
        winit: usize,
        wgpu: usize,
        monstertruck: usize,
    }

    fn fixture() -> Fixture {
        let mut g = Graph::default();
        let math = g.add("kubit-math", true);
        let api = g.add("kubit-kernel-api", true);
        let mt = g.add("kubit-kernel-monstertruck", true);
        let mesh = g.add("kubit-mesh", true);
        let render = g.add("kubit-render", true);
        let doc = g.add("kubit-doc", true);
        let engine = g.add("kubit-engine", true);
        let ui = g.add("kubit-ui", true);
        let app = g.add("kubit", true);
        let xtask = g.add("xtask", true);
        let egui = g.add("egui", false);
        let winit = g.add("winit", false);
        let wgpu = g.add("wgpu", false);
        let monstertruck = g.add("monstertruck", false);
        let serde = g.add("serde", false);
        g.depend(api, math);
        g.depend(mt, api);
        g.depend(mt, monstertruck);
        g.depend(mesh, math);
        g.depend(render, wgpu);
        g.depend(doc, api);
        g.depend(doc, serde);
        g.depend(engine, doc);
        g.depend(engine, mt);
        g.depend(ui, engine);
        g.depend(ui, egui);
        g.depend(ui, render);
        g.depend(app, ui);
        g.depend(app, winit);
        g.depend(xtask, serde);
        Fixture {
            g,
            math,
            api,
            mt,
            mesh,
            render,
            doc,
            engine,
            egui,
            winit,
            wgpu,
            monstertruck,
        }
    }

    fn violations(g: &Graph) -> Vec<String> {
        check(&map(), g).violations
    }

    #[test]
    fn valid_graph_passes() {
        let f = fixture();
        let report = check(&map(), &f.g);
        assert_eq!(report.violations, Vec::<String>::new());
        assert_eq!(report.checked, 9);
    }

    #[test]
    fn missing_crate_fails() {
        let mut f = fixture();
        f.g.add("kubit-sketch", true);
        let v = violations(&f.g);
        assert_eq!(v.len(), 1);
        assert!(
            v[0].starts_with("kubit-sketch: not in layers.toml"),
            "{v:?}"
        );
    }

    #[test]
    fn pattern_covers_backends() {
        let m = map();
        let r = m.resolve("kubit-kernel-solidcore").unwrap();
        assert!(r.kernel_backend);
        assert_eq!(r.rule.layer, 1);
        // Exact entries win over the pattern and are not backends.
        assert!(!m.resolve("kubit-kernel-api").unwrap().kernel_backend);
        assert!(m.resolve("kubit-sketch").is_none());
    }

    #[test]
    fn upward_dependency_fails() {
        let mut f = fixture();
        f.g.depend(f.math, f.doc);
        let v = violations(&f.g);
        assert_eq!(v.len(), 1);
        assert!(
            v[0].contains("kubit-math (L0) depends on kubit-doc (L2)"),
            "{v:?}"
        );
    }

    #[test]
    fn same_layer_needs_exception() {
        let mut f = fixture();
        f.g.depend(f.mesh, f.api);
        let v = violations(&f.g);
        assert_eq!(v.len(), 1);
        assert!(
            v[0].contains("kubit-mesh (L1) depends on kubit-kernel-api (L1)"),
            "{v:?}"
        );
        assert!(v[0].contains("same_layer"), "{v:?}");
        // The backend pattern lists kubit-kernel-api, so mt -> api is fine (see fixture).
    }

    #[test]
    fn ui_crate_below_l5_fails_transitively() {
        let mut f = fixture();
        let helper = f.g.add("some-helper", false);
        f.g.depend(f.doc, helper);
        f.g.depend(helper, f.winit);
        let v = violations(&f.g);
        // kubit-doc and kubit-engine (through kubit-doc) both pull winit.
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(
            v.contains(
                &"kubit-doc (L2) pulls UI crate `winit` (allowed only from L5): \
                         kubit-doc -> some-helper -> winit"
                    .to_owned()
            ),
            "{v:?}"
        );
        assert!(
            v.iter()
                .any(|s| s.contains("kubit-engine -> kubit-doc -> some-helper -> winit")),
            "{v:?}"
        );
    }

    #[test]
    fn gpu_only_in_allowed_crates() {
        let mut f = fixture();
        f.g.depend(f.doc, f.render);
        let v = violations(&f.g);
        // kubit-doc pulls wgpu through kubit-render; so does kubit-engine through kubit-doc.
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(
            v[0].contains("kubit-doc (L2) pulls GPU crate `wgpu`")
                && v[0].contains("kubit-doc -> kubit-render -> wgpu"),
            "{v:?}"
        );
    }

    #[test]
    fn gpu_direct_below_l5_fails() {
        let mut f = fixture();
        f.g.depend(f.mesh, f.wgpu);
        let v = violations(&f.g);
        assert!(
            v.iter()
                .any(|s| s.contains("kubit-mesh (L1) pulls GPU crate `wgpu`")),
            "{v:?}"
        );
        // kubit-render (gpu_allowed) also reaches wgpu via kubit-mesh, which is fine.
        assert!(!v.iter().any(|s| s.starts_with("kubit-render")), "{v:?}");
    }

    #[test]
    fn kernel_lib_outside_backend_fails() {
        let mut f = fixture();
        f.g.depend(f.api, f.monstertruck);
        let v = violations(&f.g);
        assert!(
            v.contains(
                &"kubit-kernel-api pulls kernel library `monstertruck` outside a \
                  kubit-kernel-* backend: kubit-kernel-api -> monstertruck"
                    .to_owned()
            ),
            "{v:?}"
        );
        // kubit-doc reaches it through kubit-kernel-api, which is not a backend.
        assert!(
            v.iter()
                .any(|s| s.starts_with("kubit-doc pulls kernel library")),
            "{v:?}"
        );
        // kubit-engine -> kubit-kernel-monstertruck -> monstertruck is the allowed route,
        // but engine also reaches it through doc -> api.
        assert!(
            v.iter().any(|s| s.contains("kubit-engine -> kubit-doc")),
            "{v:?}"
        );
    }

    #[test]
    fn kernel_lib_via_backend_is_allowed() {
        let f = fixture();
        // engine -> kubit-kernel-monstertruck -> monstertruck: no violation.
        assert!(violations(&f.g).is_empty());
        let _ = (f.mt, f.engine, f.egui);
    }

    #[test]
    fn dev_dependencies_are_ignored_by_construction() {
        // `Graph::from_metadata` drops dev-only edges; a graph without the edge passes.
        let f = fixture();
        assert!(violations(&f.g).is_empty());
    }

    #[test]
    fn stale_entries_warn() {
        let f = fixture();
        let report = check(&map(), &f.g);
        assert_eq!(report.warnings, Vec::<String>::new());
        let mut g = Graph::default();
        g.add("kubit-math", true);
        let report = check(&map(), &g);
        assert!(report.violations.is_empty());
        assert!(report.warnings.iter().any(|w| w.contains("`kubit-doc`")));
    }

    #[test]
    fn wasm_list() {
        let f = fixture();
        assert_eq!(
            wasm_crates(&map(), &f.g),
            [
                "kubit-doc",
                "kubit-kernel-api",
                "kubit-kernel-monstertruck",
                "kubit-math",
                "kubit-mesh",
                "kubit-render"
            ]
        );
        let _ = f.render;
    }

    #[test]
    fn same_layer_on_crate_entry() {
        let text = MAP.replace(
            "kubit-kernel-api = { layer = 1, wasm = true }",
            "kubit-kernel-api = { layer = 1, wasm = true, same_layer = [\"kubit-mesh\"] }",
        );
        let m = LayerMap::parse(&text).unwrap();
        let mut f = fixture();
        f.g.depend(f.api, f.mesh);
        assert!(check(&m, &f.g).violations.is_empty());
        // The exception is one-way: mesh -> api is still a violation.
        let mut f = fixture();
        f.g.depend(f.mesh, f.api);
        let v = check(&m, &f.g).violations;
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(
            v[0].contains("kubit-mesh (L1) depends on kubit-kernel-api"),
            "{v:?}"
        );
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let err =
            LayerMap::parse("[forbidden]\n[crates]\na = { layer = 0, wsam = true }\n").unwrap_err();
        assert!(err.contains("wsam"), "{err}");
    }

    #[test]
    fn real_layer_map_parses() {
        let text = include_str!("../../layers.toml");
        let m = LayerMap::parse(text).unwrap();
        assert!(m.resolve("kubit-kernel-truck").unwrap().kernel_backend);
    }

    #[test]
    fn glob() {
        assert!(glob_match("kubit-kernel-*", "kubit-kernel-truck"));
        assert!(glob_match("kubit-kernel-*", "kubit-kernel-"));
        assert!(!glob_match("kubit-kernel-*", "kubit-kern"));
        assert!(glob_match("a*b*c", "axxbyyc"));
        assert!(!glob_match("a*b*c", "axxc"));
        assert!(glob_match("exact", "exact"));
        assert!(!glob_match("exact", "exactly"));
        assert!(glob_match("*", "anything"));
    }
}
