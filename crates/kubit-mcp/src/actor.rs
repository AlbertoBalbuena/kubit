//! The session actor: a thread that owns the `!Send` engine `Session`.

use crate::McpError;
use kubit_doc::Author;
use kubit_engine::{CommandError, CommandOutcome, KernelFactory, Session};
use serde_json::Value;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use tokio::sync::oneshot;

/// One command as the MCP layer sees it, copied out of the registry.
#[derive(Clone, Debug)]
pub struct ToolEntry {
    /// Registry command id (`solid.box`).
    pub command: String,
    /// Display label.
    pub label: String,
    /// Registry description.
    pub description: String,
    /// The command changes the document.
    pub mutates: bool,
    /// The command reads or writes files.
    pub file_access: bool,
    /// JSON Schema of the parameters (always an object).
    pub schema: serde_json::Map<String, Value>,
}

/// Snapshot of the registry taken when the actor starts.
pub type Catalog = Vec<ToolEntry>;

enum Request {
    Execute {
        author: String,
        command: String,
        params: Value,
        reply: oneshot::Sender<Result<CommandOutcome, CommandError>>,
    },
    Document {
        reply: oneshot::Sender<Result<String, String>>,
    },
}

/// Cheap, cloneable, thread-safe handle to the session actor.
///
/// The actor stops when the last handle is dropped.
#[derive(Clone)]
pub struct SessionHandle {
    tx: mpsc::Sender<Request>,
}

fn schema_object(v: Value) -> serde_json::Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => serde_json::Map::new(),
    }
}

fn catalog_of(session: &Session) -> Catalog {
    session
        .registry()
        .iter()
        .map(|spec| {
            let mut schema = schema_object((spec.params_schema)());
            schema
                .entry("type")
                .or_insert_with(|| Value::String("object".into()));
            ToolEntry {
                command: spec.id.to_owned(),
                label: spec.label.to_owned(),
                description: spec.description.to_owned(),
                mutates: spec.mutates,
                file_access: spec.file_access,
                schema,
            }
        })
        .collect()
}

fn panic_error() -> CommandError {
    CommandError::Reference(
        "internal error: command panicked; the document may be inconsistent".into(),
    )
}

impl SessionHandle {
    /// Starts the actor thread and returns the handle with the command catalog.
    ///
    /// `make_factory` runs on the actor thread: `KernelFactory` is `!Send`
    /// (it is a `Box<dyn Fn>`), so the caller passes a `Send` closure that
    /// builds it there.
    pub fn spawn<F>(make_factory: F) -> Result<(Self, Catalog), McpError>
    where
        F: FnOnce() -> KernelFactory + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<Request>();
        let (ready_tx, ready_rx) = mpsc::channel::<Catalog>();
        std::thread::Builder::new()
            .name("kubit-session".into())
            .spawn(move || run(make_factory, rx, ready_tx))
            .map_err(|e| McpError::Actor(e.to_string()))?;
        let catalog = ready_rx
            .recv()
            .map_err(|_| McpError::Actor("actor thread failed to start".into()))?;
        Ok((Self { tx }, catalog))
    }

    /// Runs a command as an agent named `author`.
    pub async fn execute(
        &self,
        author: String,
        command: String,
        params: Value,
    ) -> Result<CommandOutcome, CommandError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Request::Execute {
                author,
                command,
                params,
                reply,
            })
            .map_err(|_| actor_gone())?;
        rx.await.map_err(|_| actor_gone())?
    }

    /// The document as `document.json` text.
    pub async fn document_json(&self) -> Result<String, String> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Request::Document { reply })
            .map_err(|_| "session actor stopped".to_owned())?;
        rx.await.map_err(|_| "session actor stopped".to_owned())?
    }
}

fn actor_gone() -> CommandError {
    CommandError::Reference("session actor stopped".into())
}

fn run<F>(make_factory: F, rx: mpsc::Receiver<Request>, ready: mpsc::Sender<Catalog>)
where
    F: FnOnce() -> KernelFactory,
{
    let mut session = Session::new(make_factory());
    if ready.send(catalog_of(&session)).is_err() {
        return;
    }
    while let Ok(request) = rx.recv() {
        match request {
            Request::Execute {
                author,
                command,
                params,
                reply,
            } => {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    session.execute_as(Author::Agent { name: author }, &command, params)
                }))
                .unwrap_or_else(|_| Err(panic_error()));
                let _ = reply.send(result);
            }
            Request::Document { reply } => {
                let result = kubit_doc::to_json(session.doc()).map_err(|e| e.to_string());
                let _ = reply.send(result);
            }
        }
    }
}
