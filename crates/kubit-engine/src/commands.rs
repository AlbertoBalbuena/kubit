//! Built-in commands of Phase 0.

use crate::error::CommandError;
use crate::model::{BoxParams, PushPullParams, validate_box};
use crate::session::{Selection, Session};
use crate::spec::{CommandOutcome, CommandSpec, Enablement, Registry, always, schema_of};
use kubit_doc::{EdgeName, FeatureId, TopoName};
use kubit_kernel_api::Kernel;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

/// No parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NoParams {}

/// A file path.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PathParams {
    /// Absolute or working-directory-relative path.
    path: PathBuf,
}

/// STL export parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StlParams {
    /// Output path (`.stl`).
    path: PathBuf,
    /// Chord tolerance in mm (default 0.05).
    #[serde(default = "default_chord")]
    chord: f64,
    /// Write ASCII instead of binary.
    #[serde(default)]
    ascii: bool,
}

fn default_chord() -> f64 {
    0.05
}

/// A body reference.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BodyParams {
    /// Body key, e.g. `F1` (the feature that created it). Default: every body.
    #[serde(default)]
    body: Option<String>,
}

/// A selection of faces and edges.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SelectParams {
    /// Persistent face names, e.g. `["F1:+Z"]`. Default: none.
    #[serde(default)]
    faces: Vec<String>,
    /// Persistent edge names, e.g. `["F1:+X|F1:+Z"]` (the two faces that meet
    /// at the edge, in either order). Default: none.
    #[serde(default)]
    edges: Vec<String>,
}

fn parse<T: for<'de> Deserialize<'de>>(
    command: &'static str,
    v: serde_json::Value,
) -> Result<T, CommandError> {
    serde_json::from_value(v).map_err(|e| CommandError::InvalidParams {
        command,
        message: e.to_string(),
    })
}

fn io(e: impl std::fmt::Display) -> CommandError {
    CommandError::Io(e.to_string())
}

fn has_bodies(s: &Session) -> Enablement {
    if s.model().bodies.is_empty() {
        Enablement::Disabled("there are no bodies")
    } else {
        Enablement::Enabled
    }
}

fn can_undo(s: &Session) -> Enablement {
    if s.can_undo() {
        Enablement::Enabled
    } else {
        Enablement::Disabled("nothing to undo")
    }
}

fn can_redo(s: &Session) -> Enablement {
    if s.can_redo() {
        Enablement::Enabled
    } else {
        Enablement::Disabled("nothing to redo")
    }
}

fn bodies_of(s: &Session, body: Option<&str>) -> Result<Vec<FeatureId>, CommandError> {
    match body {
        None => Ok(s.model().bodies.keys().copied().collect()),
        Some(text) => {
            let id: FeatureId = text.parse().map_err(CommandError::Reference)?;
            if s.model().bodies.contains_key(&id) {
                Ok(vec![id])
            } else {
                Err(CommandError::Reference(format!("body {id}")))
            }
        }
    }
}

// --- modelling --------------------------------------------------------------

fn run_box(s: &mut Session, v: serde_json::Value) -> Result<CommandOutcome, CommandError> {
    let p: BoxParams = parse("solid.box", v)?;
    validate_box(&p)?;
    let canonical = serde_json::to_value(&p).map_err(|e| CommandError::Format(e.to_string()))?;
    let id = s.add_feature("solid.box", canonical)?;
    let [x, y, z] = p.size;
    Ok(CommandOutcome {
        summary: format!("Created box {id} ({x} × {y} × {z} mm)"),
        data: json!({ "feature": id.to_string(), "body": id.to_string() }),
    })
}

fn run_push_pull(s: &mut Session, v: serde_json::Value) -> Result<CommandOutcome, CommandError> {
    let p: PushPullParams = parse("face.push_pull", v)?;
    let canonical = serde_json::to_value(&p).map_err(|e| CommandError::Format(e.to_string()))?;
    let id = s.add_feature("face.push_pull", canonical)?;
    Ok(CommandOutcome {
        summary: format!("Moved face {} by {} mm ({id})", p.face, p.distance),
        data: json!({ "feature": id.to_string(), "face": p.face }),
    })
}

// --- edit -------------------------------------------------------------------

fn run_undo(s: &mut Session, v: serde_json::Value) -> Result<CommandOutcome, CommandError> {
    let _: NoParams = parse("edit.undo", v)?;
    s.undo();
    Ok(CommandOutcome::text("Undone"))
}

fn run_redo(s: &mut Session, v: serde_json::Value) -> Result<CommandOutcome, CommandError> {
    let _: NoParams = parse("edit.redo", v)?;
    s.redo();
    Ok(CommandOutcome::text("Redone"))
}

fn run_select(s: &mut Session, v: serde_json::Value) -> Result<CommandOutcome, CommandError> {
    let p: SelectParams = parse("selection.set", v)?;
    let mut sel = Selection::default();
    for f in &p.faces {
        let name: TopoName = f
            .parse()
            .map_err(|e: kubit_doc::NameParseError| CommandError::Reference(e.to_string()))?;
        if s.model().body_of_face(&name).is_none() {
            return Err(CommandError::Reference(format!("face `{name}`")));
        }
        sel.faces.insert(name);
    }
    for e in &p.edges {
        let name: EdgeName = e
            .parse()
            .map_err(|e: kubit_doc::EdgeNameParseError| CommandError::Reference(e.to_string()))?;
        if s.model().body_of_edge(&name).is_none() {
            return Err(CommandError::Reference(format!("edge `{name}`")));
        }
        sel.edges.insert(name);
    }
    let (faces, edges) = (sel.faces.len(), sel.edges.len());
    s.set_selection(sel);
    Ok(CommandOutcome::text(format!(
        "{faces} face(s), {edges} edge(s) selected"
    )))
}

// --- documents and files ----------------------------------------------------

fn run_new(s: &mut Session, v: serde_json::Value) -> Result<CommandOutcome, CommandError> {
    let _: NoParams = parse("doc.new", v)?;
    s.new_document();
    Ok(CommandOutcome::text("New document"))
}

fn run_save(s: &mut Session, v: serde_json::Value) -> Result<CommandOutcome, CommandError> {
    let p: PathParams = parse("doc.save", v)?;
    let json = p
        .path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    let bytes = if json {
        kubit_doc::to_json(s.doc())
            .map_err(|e| CommandError::Format(e.to_string()))?
            .into_bytes()
    } else {
        kubit_doc::to_kubit(s.doc(), None).map_err(|e| CommandError::Format(e.to_string()))?
    };
    std::fs::write(&p.path, bytes).map_err(io)?;
    Ok(CommandOutcome::text(format!("Saved {}", p.path.display())))
}

fn run_open(s: &mut Session, v: serde_json::Value) -> Result<CommandOutcome, CommandError> {
    let p: PathParams = parse("doc.open", v)?;
    let bytes = std::fs::read(&p.path).map_err(io)?;
    let json = p
        .path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    let doc = if json {
        let text = String::from_utf8(bytes).map_err(|e| CommandError::Format(e.to_string()))?;
        kubit_doc::from_json(&text)
    } else {
        kubit_doc::from_kubit(&bytes)
    }
    .map_err(|e| CommandError::Format(e.to_string()))?;
    s.load(doc);
    let failed: Vec<String> = s
        .model()
        .status
        .iter()
        .filter(|(_, st)| matches!(st, crate::model::FeatureStatus::Failed(_)))
        .map(|(id, _)| id.to_string())
        .collect();
    let summary = if failed.is_empty() {
        format!("Opened {}", p.path.display())
    } else {
        format!(
            "Opened {} with failing features: {}",
            p.path.display(),
            failed.join(", ")
        )
    };
    Ok(CommandOutcome {
        summary,
        data: json!({ "failed_features": failed }),
    })
}

fn run_export_step(s: &mut Session, v: serde_json::Value) -> Result<CommandOutcome, CommandError> {
    let p: PathParams = parse("file.export_step", v)?;
    let bodies = s.kernel_bodies();
    let text = s.kernel().export_step(&bodies)?;
    std::fs::write(&p.path, text).map_err(io)?;
    Ok(CommandOutcome::text(format!(
        "Exported {} body(ies) to {}",
        bodies.len(),
        p.path.display()
    )))
}

fn run_export_stl(s: &mut Session, v: serde_json::Value) -> Result<CommandOutcome, CommandError> {
    let p: StlParams = parse("file.export_stl", v)?;
    let keys: Vec<FeatureId> = s.model().bodies.keys().copied().collect();
    let mut mesh = kubit_mesh::TriMesh::default();
    for k in keys {
        mesh.append(s.body_mesh(k, p.chord)?);
    }
    let bytes = if p.ascii {
        kubit_io::stl::write_ascii(&mesh, "kubit").map(String::into_bytes)
    } else {
        kubit_io::stl::write_binary(&mesh)
    }
    .map_err(|e| CommandError::Format(e.to_string()))?;
    std::fs::write(&p.path, bytes).map_err(io)?;
    Ok(CommandOutcome::text(format!(
        "Exported {} triangles to {}",
        mesh.triangles.len(),
        p.path.display()
    )))
}

// --- queries ----------------------------------------------------------------

fn run_inspect(s: &mut Session, v: serde_json::Value) -> Result<CommandOutcome, CommandError> {
    let _: NoParams = parse("doc.inspect", v)?;
    let doc = s.doc();
    let features: Vec<serde_json::Value> = doc
        .features
        .iter()
        .map(|f| {
            json!({
                "id": f.id.to_string(),
                "command": f.command,
                "params": f.params,
                "author": f.author,
                "status": s.model().status.get(&f.id),
            })
        })
        .collect();
    let mut bodies = Vec::new();
    for (key, b) in &s.model().bodies {
        let mass = s.kernel().mass_properties(b.kernel)?;
        bodies.push(json!({
            "body": key.to_string(),
            "faces": b.names.len(),
            "edges": b.edges.len(),
            "volume_mm3": mass.volume,
            "area_mm2": mass.area,
            "tip": b.tip.to_string(),
        }));
    }
    let info = s.kernel_info();
    Ok(CommandOutcome {
        summary: format!("{} feature(s), {} body(ies)", features.len(), bodies.len()),
        data: json!({
            "kernel": { "name": info.name, "version": info.version },
            "features": features,
            "bodies": bodies,
            "selection": s.selection().faces.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "selected_edges": s.selection().edges.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "can_undo": s.can_undo(),
            "can_redo": s.can_redo(),
        }),
    })
}

fn run_faces(s: &mut Session, v: serde_json::Value) -> Result<CommandOutcome, CommandError> {
    let p: BodyParams = parse("body.faces", v)?;
    let mut out = Vec::new();
    for key in bodies_of(s, p.body.as_deref())? {
        let Some(b) = s.model().bodies.get(&key) else {
            continue;
        };
        for (name, face) in &b.names {
            let info = s.kernel().face_info(*face)?;
            out.push(json!({
                "body": key.to_string(),
                "face": name.to_string(),
                "surface": format!("{:?}", info.surface),
                "normal": info.normal,
                "centroid": info.centroid,
                "area_mm2": info.area,
            }));
        }
    }
    Ok(CommandOutcome {
        summary: format!("{} face(s)", out.len()),
        data: json!({ "faces": out }),
    })
}

fn run_measure(s: &mut Session, v: serde_json::Value) -> Result<CommandOutcome, CommandError> {
    let p: BodyParams = parse("body.measure", v)?;
    let mut out = Vec::new();
    let mut total = 0.0;
    for key in bodies_of(s, p.body.as_deref())? {
        let Some(b) = s.model().bodies.get(&key) else {
            continue;
        };
        let m = s.kernel().mass_properties(b.kernel)?;
        let validity = s.kernel().check(b.kernel)?;
        total += m.volume;
        out.push(json!({
            "body": key.to_string(),
            "volume_mm3": m.volume,
            "area_mm2": m.area,
            "centroid": m.centroid,
            "exact": m.exact,
            "valid": validity.ok,
        }));
    }
    Ok(CommandOutcome {
        summary: format!("Total volume {total:.3} mm³"),
        data: json!({ "bodies": out }),
    })
}

/// Shorthand to declare a command.
#[allow(clippy::too_many_arguments)]
const fn spec(
    id: &'static str,
    label: &'static str,
    description: &'static str,
    icon: Option<&'static str>,
    shortcut: Option<&'static str>,
    toolbar_group: Option<&'static str>,
    mutates: bool,
    file_access: bool,
    params_schema: fn() -> serde_json::Value,
    enabled: fn(&Session) -> Enablement,
    run: crate::spec::RunFn,
) -> CommandSpec {
    CommandSpec {
        id,
        label,
        description,
        icon,
        shortcut,
        toolbar_group,
        mutates,
        file_access,
        params_schema,
        enabled,
        run,
    }
}

/// The Phase 0 command set.
pub fn builtin() -> Registry {
    let mut r = Registry::new();
    for c in [
        spec(
            "solid.box",
            "cmd.solid.box",
            "Create an axis-aligned box from its minimum corner `origin` and `size` [x, y, z] in mm.",
            Some("box"),
            Some("B"),
            Some("create"),
            true,
            false,
            schema_of::<BoxParams>,
            always,
            run_box,
        ),
        spec(
            "face.push_pull",
            "cmd.face.push_pull",
            "Move a face along its outward normal by `distance` mm (negative pushes in). Faces are named like `F1:+Z`; list them with `body.faces`.",
            Some("push_pull"),
            Some("P"),
            Some("modify"),
            true,
            false,
            schema_of::<PushPullParams>,
            has_bodies,
            run_push_pull,
        ),
        spec(
            "edit.undo",
            "cmd.edit.undo",
            "Undo the last change.",
            Some("undo"),
            Some("Ctrl+Z"),
            Some("edit"),
            true,
            false,
            schema_of::<NoParams>,
            can_undo,
            run_undo,
        ),
        spec(
            "edit.redo",
            "cmd.edit.redo",
            "Redo the last undone change.",
            Some("redo"),
            Some("Ctrl+Shift+Z"),
            Some("edit"),
            true,
            false,
            schema_of::<NoParams>,
            can_redo,
            run_redo,
        ),
        spec(
            "selection.set",
            "cmd.selection.set",
            "Replace the selection with the given faces and edges.",
            None,
            None,
            None,
            true,
            false,
            schema_of::<SelectParams>,
            always,
            run_select,
        ),
        spec(
            "doc.new",
            "cmd.doc.new",
            "Start a new empty document (discards undo history).",
            Some("new"),
            Some("Ctrl+N"),
            None,
            true,
            false,
            schema_of::<NoParams>,
            always,
            run_new,
        ),
        spec(
            "doc.open",
            "cmd.doc.open",
            "Open a .kubit (or .json) document from `path`.",
            Some("open"),
            Some("Ctrl+O"),
            None,
            true,
            true,
            schema_of::<PathParams>,
            always,
            run_open,
        ),
        spec(
            "doc.save",
            "cmd.doc.save",
            "Save the document to `path` (.kubit container, or plain JSON if the path ends in .json).",
            Some("save"),
            Some("Ctrl+S"),
            None,
            false,
            true,
            schema_of::<PathParams>,
            always,
            run_save,
        ),
        spec(
            "file.export_step",
            "cmd.file.export_step",
            "Export every body to a STEP file at `path` (mm).",
            Some("export"),
            None,
            None,
            false,
            true,
            schema_of::<PathParams>,
            has_bodies,
            run_export_step,
        ),
        spec(
            "file.export_stl",
            "cmd.file.export_stl",
            "Export every body to an STL file at `path` (mm), with chord tolerance `chord`.",
            Some("export"),
            None,
            None,
            false,
            true,
            schema_of::<StlParams>,
            has_bodies,
            run_export_stl,
        ),
        spec(
            "doc.inspect",
            "cmd.doc.inspect",
            "Describe the document: features with status and author, bodies with volume, selection, undo state.",
            None,
            None,
            None,
            false,
            false,
            schema_of::<NoParams>,
            always,
            run_inspect,
        ),
        spec(
            "body.faces",
            "cmd.body.faces",
            "List the faces of a body (or all bodies) with persistent names, surface type, normal, centroid and area.",
            None,
            None,
            None,
            false,
            false,
            schema_of::<BodyParams>,
            always,
            run_faces,
        ),
        spec(
            "body.measure",
            "cmd.body.measure",
            "Volume, area, centroid and validity of a body (or all bodies).",
            Some("measure"),
            None,
            None,
            false,
            false,
            schema_of::<BodyParams>,
            always,
            run_measure,
        ),
    ] {
        r.register(c);
    }
    r
}
