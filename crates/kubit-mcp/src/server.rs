//! The rmcp `ServerHandler` generated from the command catalog.

use crate::actor::{Catalog, SessionHandle};
use crate::{McpError, ServerOptions};
use kubit_engine::{CommandError, KernelFactory};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListResourcesResult, ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams,
    ReadResourceResponse, ReadResourceResult, Resource, ResourceContents, ServerCapabilities,
    ServerConfig, Tool, ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::transport::IntoTransport;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

const DOCUMENT_URI: &str = "kubit://document";
const MODEL_URI: &str = "kubit://model";
/// Agent name when the client sent none; names are capped to keep the timeline sane.
const DEFAULT_AGENT: &str = "mcp-client";
const MAX_AGENT_NAME: usize = 128;

/// MCP tool name for a command id (`.` is not portable across LLM APIs).
fn tool_name(command: &str) -> String {
    command.replace('.', "_")
}

/// The MCP server: tools generated from the registry, plus two resources.
#[derive(Clone)]
pub struct KubitServer {
    handle: SessionHandle,
    tools: Arc<Vec<Tool>>,
    /// Exposed tool name → command id (the reverse map).
    commands: Arc<BTreeMap<String, String>>,
}

impl KubitServer {
    /// Builds the server over a running session actor.
    pub fn new(
        handle: SessionHandle,
        catalog: Catalog,
        options: ServerOptions,
    ) -> Result<Self, McpError> {
        let mut commands = BTreeMap::new();
        let mut tools = Vec::new();
        for entry in catalog {
            let name = tool_name(&entry.command);
            if let Some(previous) = commands.insert(name.clone(), entry.command.clone()) {
                return Err(McpError::NameCollision(previous, entry.command, name));
            }
            if !options.exposes(entry.mutates, entry.file_access) {
                commands.remove(&name);
                continue;
            }
            let mut description = entry.description.clone();
            if entry.mutates {
                description.push_str("\n\nMutates the document.");
            }
            if entry.file_access {
                description.push_str("\n\nReads or writes files on the host.");
            }
            let annotations =
                ToolAnnotations::new().read_only(!entry.mutates && !entry.file_access);
            tools.push(
                Tool::new(name, description, Arc::new(entry.schema))
                    .with_title(entry.label)
                    .with_annotations(annotations),
            );
        }
        Ok(Self {
            handle,
            tools: Arc::new(tools),
            commands: Arc::new(commands),
        })
    }

    /// Names of the exposed tools (for diagnostics and tests).
    pub fn tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.name.to_string()).collect()
    }

    async fn read_model(&self) -> Result<String, String> {
        let outcome = self
            .handle
            .execute(DEFAULT_AGENT.into(), "doc.inspect".into(), Value::Null)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&outcome.data).map_err(|e| e.to_string())
    }
}

fn command_error(e: &CommandError) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(e.to_string())])
}

fn agent_name(context: &RequestContext<RoleServer>) -> String {
    let name = context
        .peer
        .peer_info()
        .map(|info| info.client_info.name.clone())
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_AGENT.to_owned());
    name.chars().take(MAX_AGENT_NAME).collect()
}

impl ServerHandler for KubitServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new("kubit", env!("CARGO_PKG_VERSION")))
        .with_instructions(
            "Kubit CAD. Every tool is a Kubit command. Faces are named like `F1:+Z`; \
             use body_faces to list them. Dimensions are millimetres and radians.",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(self.tools.as_ref().clone()))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools.iter().find(|t| t.name == name).cloned()
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let Some(command) = self.commands.get(request.name.as_ref()) else {
            return Err(ErrorData::invalid_params(
                format!("unknown tool `{}`", request.name),
                None,
            ));
        };
        let params = request.arguments.map_or(Value::Null, Value::Object);
        let result = match self
            .handle
            .execute(agent_name(&context), command.clone(), params)
            .await
        {
            Ok(outcome) => {
                let mut content = vec![ContentBlock::text(outcome.summary)];
                let mut result = match outcome.data {
                    Value::Null => CallToolResult::success(content),
                    data => {
                        // Structured content must be an object (MCP spec).
                        let structured = match data {
                            Value::Object(_) => data,
                            other => serde_json::json!({ "result": other }),
                        };
                        // Spec: also serialise it as text for clients that ignore it.
                        content.push(ContentBlock::text(structured.to_string()));
                        let mut r = CallToolResult::success(content);
                        r.structured_content = Some(structured);
                        r
                    }
                };
                result.is_error = Some(false);
                result
            }
            Err(e) => command_error(&e),
        };
        Ok(result.into())
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new(DOCUMENT_URI, "document")
                .with_title("Document")
                .with_description("The Kubit document: parameters and feature timeline.")
                .with_mime_type("application/json"),
            Resource::new(MODEL_URI, "model")
                .with_title("Model")
                .with_description("Derived state: features, bodies, volumes, selection.")
                .with_mime_type("application/json"),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let text = match request.uri.as_str() {
            DOCUMENT_URI => self.handle.document_json().await,
            MODEL_URI => self.read_model().await,
            other => {
                return Err(ErrorData::resource_not_found(
                    format!("unknown resource `{other}`"),
                    None,
                ));
            }
        }
        .map_err(|e| ErrorData::internal_error(e, None))?;
        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(text, request.uri).with_mime_type("application/json"),
        ])
        .into())
    }
}

/// Serves MCP over any rmcp transport until the peer disconnects.
///
/// `make_factory` runs on the session actor thread (see [`SessionHandle::spawn`]).
pub async fn serve<F, T, E, A>(
    make_factory: F,
    options: ServerOptions,
    transport: T,
) -> Result<(), McpError>
where
    F: FnOnce() -> KernelFactory + Send + 'static,
    T: IntoTransport<RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let (handle, catalog) = SessionHandle::spawn(make_factory)?;
    let server = KubitServer::new(handle, catalog, options)?;
    let running = server
        .serve(transport)
        .await
        .map_err(|e| McpError::Transport(e.to_string()))?;
    running
        .waiting()
        .await
        .map_err(|e| McpError::Transport(e.to_string()))?;
    Ok(())
}

/// Serves MCP over stdin/stdout. stdout carries protocol frames only: log to stderr.
pub async fn serve_stdio<F>(make_factory: F, options: ServerOptions) -> Result<(), McpError>
where
    F: FnOnce() -> KernelFactory + Send + 'static,
{
    serve(make_factory, options, rmcp::transport::io::stdio()).await
}
