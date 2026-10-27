//! The session: document + kernel + derived model + undo/redo + selection.
//!
//! Every mutating command is a transaction: it works on copies of the document
//! and the model and commits only on success, so a failure changes nothing.
//! Undo/redo restore snapshots of (document, model, selection). Kernel bodies
//! are never edited in place (operations return new bodies), so a snapshot stays
//! valid; bodies no snapshot references are freed after each commit.

use crate::commands;
use crate::error::CommandError;
use crate::model::{FeatureStatus, Model, apply_feature, canonical_params, is_feature};
use crate::spec::{CommandOutcome, CommandSpec, Enablement, Registry};
use kubit_doc::{Author, Document, EdgeName, FeatureId, TopoName};
use kubit_kernel_api::{BodyId, Guarded, Kernel, KernelInfo, TriMesh};
use kubit_mesh::{EdgePolyline, EdgeTag, FaceTag};
use std::collections::{BTreeMap, BTreeSet};

/// Builds a fresh kernel (used on open and after a kernel panic).
pub type KernelFactory = Box<dyn Fn() -> Box<dyn Kernel>>;

/// Selected entities, by persistent name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// Selected faces.
    pub faces: BTreeSet<TopoName>,
    /// Selected edges.
    pub edges: BTreeSet<EdgeName>,
}

impl Selection {
    /// Nothing selected.
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty() && self.edges.is_empty()
    }
}

/// Notifications for front-ends (drained with `Session::take_events`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The document or the model changed (redraw, refresh panels).
    DocumentChanged,
    /// The selection changed.
    SelectionChanged,
    /// The kernel panicked and was rebuilt from the document.
    KernelRecovered,
}

#[derive(Clone, Debug)]
struct Snapshot {
    doc: Document,
    model: Model,
    selection: Selection,
}

/// Maximum undo depth.
pub const UNDO_LIMIT: usize = 200;

/// A modelling session.
pub struct Session {
    doc: Document,
    model: Model,
    selection: Selection,
    kernel: Guarded<Box<dyn Kernel>>,
    factory: KernelFactory,
    registry: Registry,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    allocated: BTreeSet<BodyId>,
    author: Author,
    events: Vec<Event>,
    mesh_cache: BTreeMap<(BodyId, u64), TriMesh>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("doc", &self.doc)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

impl Session {
    /// A session with an empty document and the built-in commands.
    pub fn new(factory: KernelFactory) -> Self {
        let kernel = Guarded::new(factory());
        Self {
            doc: Document::new(),
            model: Model::default(),
            selection: Selection::default(),
            kernel,
            factory,
            registry: commands::builtin(),
            undo: Vec::new(),
            redo: Vec::new(),
            allocated: BTreeSet::new(),
            author: Author::User,
            events: Vec::new(),
            mesh_cache: BTreeMap::new(),
        }
    }

    // --- the single entry point ------------------------------------------------

    /// Runs a command by id with JSON parameters, as the current author.
    pub fn execute(
        &mut self,
        id: &str,
        params: serde_json::Value,
    ) -> Result<CommandOutcome, CommandError> {
        let spec: CommandSpec = *self
            .registry
            .get(id)
            .ok_or_else(|| CommandError::UnknownCommand(id.to_owned()))?;
        // Selection-gated commands name their targets in the parameters, so a
        // script, the CLI or MCP does not need a selection to run them.
        let gated = !matches!(spec.id, "edge.fillet" | "edge.chamfer" | "body.shell");
        if let (true, Enablement::Disabled(reason)) = (gated, (spec.enabled)(self)) {
            return Err(CommandError::Disabled {
                command: spec.id,
                reason,
            });
        }
        let params = if params.is_null() {
            serde_json::json!({})
        } else {
            params
        };
        (spec.run)(self, params)
    }

    /// Runs a command as `author` (MCP passes the agent; scripts the script).
    pub fn execute_as(
        &mut self,
        author: Author,
        id: &str,
        params: serde_json::Value,
    ) -> Result<CommandOutcome, CommandError> {
        let previous = std::mem::replace(&mut self.author, author);
        let r = self.execute(id, params);
        self.author = previous;
        r
    }

    // --- read access ------------------------------------------------------------

    /// The document.
    pub fn doc(&self) -> &Document {
        &self.doc
    }

    /// The derived model.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// The command registry.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The selection.
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    /// The (guarded) kernel, for queries.
    pub fn kernel(&self) -> &Guarded<Box<dyn Kernel>> {
        &self.kernel
    }

    /// Backend identity.
    pub fn kernel_info(&self) -> KernelInfo {
        self.kernel.info()
    }

    /// Whether undo / redo have something to do.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// See [`Session::can_undo`].
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Takes the pending events.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Mesh of a document body (cached per kernel body and chord tolerance).
    /// Triangle tags are kernel face ids; map them with [`Session::face_name`].
    /// `TriMesh::edges` holds the B-rep edges; map their tags with
    /// [`Session::edge_name`].
    pub fn body_mesh(&mut self, body: FeatureId, chord: f64) -> Result<&TriMesh, CommandError> {
        let kb = self
            .model
            .bodies
            .get(&body)
            .map(|b| b.kernel)
            .ok_or_else(|| CommandError::Reference(format!("body {body}")))?;
        let key = (kb, chord.to_bits());
        if !self.mesh_cache.contains_key(&key) {
            let mesh = self.kernel.tessellate(kb, chord)?;
            self.mesh_cache.insert(key, mesh);
        }
        self.mesh_cache
            .get(&key)
            .ok_or_else(|| CommandError::Reference(format!("mesh of {body}")))
    }

    /// B-rep edge polylines of a document body (from the cached mesh at
    /// `chord`). Tags are kernel edge ids; map them with [`Session::edge_name`].
    pub fn body_edges(
        &mut self,
        body: FeatureId,
        chord: f64,
    ) -> Result<&[EdgePolyline], CommandError> {
        Ok(self.body_mesh(body, chord)?.edges.as_slice())
    }

    /// Persistent name of the face behind a mesh tag.
    pub fn face_name(&self, body: FeatureId, tag: FaceTag) -> Option<TopoName> {
        let b = self.model.bodies.get(&body)?;
        b.name_of(kubit_kernel_api::FaceId(tag.0)).cloned()
    }

    /// Persistent name of the edge behind a mesh edge tag; `None` when the edge
    /// has no unambiguous name (ADR-0004).
    pub fn edge_name(&self, body: FeatureId, tag: EdgeTag) -> Option<EdgeName> {
        let b = self.model.bodies.get(&body)?;
        b.edge_name_of(kubit_kernel_api::EdgeId(tag.0)).cloned()
    }

    // --- transactions (used by command implementations) ------------------------

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            doc: self.doc.clone(),
            model: self.model.clone(),
            selection: self.selection.clone(),
        }
    }

    fn track(&mut self) {
        let live: Vec<BodyId> = self.model.kernel_bodies().collect();
        self.allocated.extend(live);
    }

    /// Frees kernel bodies no snapshot references.
    fn collect_garbage(&mut self) {
        let mut referenced: BTreeSet<BodyId> = self.model.kernel_bodies().collect();
        for s in self.undo.iter().chain(&self.redo) {
            referenced.extend(s.model.kernel_bodies());
        }
        let dead: Vec<BodyId> = self.allocated.difference(&referenced).copied().collect();
        for b in dead {
            let _ = self.kernel.delete_body(b);
            self.allocated.remove(&b);
        }
        self.mesh_cache.retain(|(b, _), _| referenced.contains(b));
    }

    fn commit(&mut self, before: Snapshot) {
        self.undo.push(before);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.track();
        self.collect_garbage();
        self.events.push(Event::DocumentChanged);
    }

    /// Appends a timeline feature and applies it, as one undoable transaction.
    pub fn add_feature(
        &mut self,
        command: &'static str,
        params: serde_json::Value,
    ) -> Result<FeatureId, CommandError> {
        if !is_feature(command) {
            return Err(CommandError::UnknownFeature {
                feature: "new".into(),
                command: command.into(),
            });
        }
        let before = self.snapshot();
        let mut doc = self.doc.clone();
        let mut model = self.model.clone();
        let id = doc.push_feature(command, params, self.author.clone());
        let feature = doc
            .feature(id)
            .cloned()
            .ok_or_else(|| CommandError::Reference(id.to_string()))?;
        let result = apply_feature(&mut self.kernel, &mut model, &feature);
        if let Err(e) = result {
            self.recover_if_poisoned();
            // Bodies created before the failure are unreferenced: free them.
            self.collect_garbage_after_failure(&model);
            return Err(e);
        }
        model.status.insert(id, FeatureStatus::Ok);
        // Drop selected names the operation made disappear.
        self.selection
            .faces
            .retain(|n| model.body_of_face(n).is_some());
        self.selection
            .edges
            .retain(|n| model.body_of_edge(n).is_some());
        self.doc = doc;
        self.model = model;
        self.commit(before);
        Ok(id)
    }

    /// Replaces the params of feature `id` and rebuilds the document, as one
    /// undoable transaction. Later features that fail after the change are recorded
    /// in `Model::status`; if `id` itself fails, nothing changes.
    pub fn edit_feature(
        &mut self,
        id: FeatureId,
        params: serde_json::Value,
    ) -> Result<(), CommandError> {
        self.rebuild_transaction(id, |doc| {
            let feature = doc
                .features
                .iter_mut()
                .find(|f| f.id == id)
                .ok_or_else(|| CommandError::Reference(id.to_string()))?;
            feature.params = params;
            Ok(())
        })
    }

    /// `feature.edit`: like [`Session::edit_feature`], after checking `params`
    /// against the feature's own command schema. Stores the canonical form.
    pub fn edit_feature_checked(
        &mut self,
        id: FeatureId,
        params: serde_json::Value,
    ) -> Result<(), CommandError> {
        let command = self
            .doc
            .feature(id)
            .map(|f| f.command.clone())
            .ok_or_else(|| CommandError::Reference(id.to_string()))?;
        let canonical = canonical_params(&command, &params)?;
        self.edit_feature(id, canonical)
    }

    /// Suppresses or restores feature `id` and rebuilds. Restoring a feature that
    /// then fails changes nothing and returns the error.
    pub fn suppress_feature(
        &mut self,
        id: FeatureId,
        suppressed: bool,
    ) -> Result<(), CommandError> {
        if self.doc.feature(id).is_none() {
            return Err(CommandError::Reference(id.to_string()));
        }
        if self
            .doc
            .feature(id)
            .is_some_and(|f| f.suppressed == suppressed)
        {
            return Ok(());
        }
        self.rebuild_transaction(id, |doc| {
            let feature = doc
                .features
                .iter_mut()
                .find(|f| f.id == id)
                .ok_or_else(|| CommandError::Reference(id.to_string()))?;
            feature.suppressed = suppressed;
            Ok(())
        })
    }

    /// Removes feature `id` and rebuilds; features that depended on it end up
    /// `Failed`. When `id` is the rollback marker, the marker moves to the
    /// previous feature (or is cleared if there is none).
    pub fn delete_feature(&mut self, id: FeatureId) -> Result<(), CommandError> {
        self.rebuild_transaction(id, |doc| {
            let at = doc
                .features
                .iter()
                .position(|f| f.id == id)
                .ok_or_else(|| CommandError::Reference(id.to_string()))?;
            doc.features.remove(at);
            if doc.rollback == Some(id) {
                doc.rollback = at
                    .checked_sub(1)
                    .and_then(|i| doc.features.get(i))
                    .map(|f| f.id);
            }
            Ok(())
        })
    }

    /// Moves the rollback marker after `after` (`None`: the end of the timeline)
    /// and rebuilds.
    pub fn set_rollback(&mut self, after: Option<FeatureId>) -> Result<(), CommandError> {
        if let Some(id) = after
            && self.doc.feature(id).is_none()
        {
            return Err(CommandError::Reference(id.to_string()));
        }
        if self.doc.rollback == after {
            return Ok(());
        }
        self.rebuild_transaction(FeatureId(0), |doc| {
            doc.rollback = after;
            Ok(())
        })
    }

    /// Applies `change` to a copy of the document, rebuilds it and commits one
    /// undo step. Fails, changing nothing, when `change` fails, when the kernel
    /// panicked, or when feature `subject` (if it still exists and is applied)
    /// is `Failed` after the rebuild.
    fn rebuild_transaction(
        &mut self,
        subject: FeatureId,
        change: impl FnOnce(&mut Document) -> Result<(), CommandError>,
    ) -> Result<(), CommandError> {
        let before = self.snapshot();
        let mut doc = self.doc.clone();
        change(&mut doc)?;
        let model = Self::rebuild(&mut self.kernel, &doc);
        if self.kernel.is_poisoned() {
            self.recover_if_poisoned();
            return Err(CommandError::Reference(
                "the kernel failed while rebuilding; nothing was changed".into(),
            ));
        }
        if let Some(FeatureStatus::Failed(message)) = model.status.get(&subject) {
            let message = message.clone();
            self.collect_garbage_after_failure(&model);
            return Err(CommandError::Reference(message));
        }
        self.selection
            .faces
            .retain(|n| model.body_of_face(n).is_some());
        self.selection
            .edges
            .retain(|n| model.body_of_edge(n).is_some());
        self.doc = doc;
        self.model = model;
        self.commit(before);
        Ok(())
    }

    fn collect_garbage_after_failure(&mut self, scratch: &Model) {
        let keep: BTreeSet<BodyId> = self.model.kernel_bodies().collect();
        for b in scratch.kernel_bodies() {
            if !keep.contains(&b) && !self.allocated.contains(&b) {
                let _ = self.kernel.delete_body(b);
            }
        }
    }

    /// Restores the previous state.
    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.redo.push(current);
        self.restore(prev);
        true
    }

    /// Re-applies an undone change.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.undo.push(current);
        self.restore(next);
        true
    }

    fn restore(&mut self, s: Snapshot) {
        self.doc = s.doc;
        self.model = s.model;
        self.selection = s.selection;
        self.events.push(Event::DocumentChanged);
        self.events.push(Event::SelectionChanged);
    }

    /// Replaces the selection (not part of undo history on its own).
    pub fn set_selection(&mut self, selection: Selection) {
        if selection != self.selection {
            self.selection = selection;
            self.events.push(Event::SelectionChanged);
        }
    }

    // --- whole-document operations ----------------------------------------------

    /// Replays a document on a fresh kernel, up to the rollback marker if any.
    /// Failing features are recorded in `Model::status` and skipped; the rest
    /// still rebuild; features after the marker are reported `Suppressed`.
    pub fn load(&mut self, doc: Document) {
        self.kernel.reset((self.factory)());
        self.allocated.clear();
        self.mesh_cache.clear();
        self.undo.clear();
        self.redo.clear();
        self.selection = Selection::default();
        self.model = Self::rebuild(&mut self.kernel, &doc);
        self.doc = doc;
        self.track();
        self.events.push(Event::DocumentChanged);
        self.events.push(Event::SelectionChanged);
    }

    fn rebuild(kernel: &mut Guarded<Box<dyn Kernel>>, doc: &Document) -> Model {
        let mut model = Model::default();
        let mut rolled_back = false;
        for f in &doc.features {
            if rolled_back {
                model.status.insert(f.id, FeatureStatus::Suppressed);
                continue;
            }
            if doc.rollback == Some(f.id) {
                rolled_back = true;
            }
            if f.suppressed {
                model.status.insert(f.id, FeatureStatus::Suppressed);
                continue;
            }
            let mut attempt = model.clone();
            match apply_feature(kernel, &mut attempt, f) {
                Ok(()) => {
                    model = attempt;
                    model.status.insert(f.id, FeatureStatus::Ok);
                }
                Err(e) => {
                    model
                        .status
                        .insert(f.id, FeatureStatus::Failed(e.to_string()));
                }
            }
        }
        model
    }

    /// After a kernel panic the guard poisons the kernel; rebuild it from the
    /// current document (undo history is dropped: its bodies are gone).
    fn recover_if_poisoned(&mut self) {
        if self.kernel.is_poisoned() {
            let doc = self.doc.clone();
            self.load(doc);
            self.events.push(Event::KernelRecovered);
        }
    }

    /// Starts a new empty document.
    pub fn new_document(&mut self) {
        self.load(Document::new());
    }

    /// Kernel bodies of every document body, in key order.
    pub fn kernel_bodies(&self) -> Vec<BodyId> {
        self.model.kernel_bodies().collect()
    }
}
