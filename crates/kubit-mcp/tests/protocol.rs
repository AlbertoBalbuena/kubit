#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
//! Protocol integration tests: a real rmcp client against the server over an
//! in-memory duplex transport, on the exact reference kernel.

use kubit_kernel_api::Kernel;
use kubit_kernel_api::testing::BoxKernel;
use kubit_mcp::{ServerOptions, serve};
use rmcp::ServiceExt;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientCapabilities, ClientConfig, Implementation,
    ReadResourceRequestParams, ResourceContents,
};
use rmcp::service::{RoleClient, RunningService};
use serde_json::{Value, json};

type Client = RunningService<RoleClient, ClientConfig>;

fn factory() -> kubit_engine::KernelFactory {
    Box::new(|| Box::new(BoxKernel::new()) as Box<dyn Kernel>)
}

async fn connect(options: ServerOptions) -> Client {
    let (server_io, client_io) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        let _ = serve(factory, options, server_io).await;
    });
    let info = ClientConfig::new(
        ClientCapabilities::default(),
        Implementation::new("test-agent", "1.0"),
    );
    info.serve(client_io).await.unwrap()
}

async fn call(c: &Client, tool: &str, args: Value) -> CallToolResult {
    let mut p = CallToolRequestParams::new(tool.to_owned());
    if let Value::Object(m) = args {
        p = p.with_arguments(m);
    }
    c.call_tool(p).await.unwrap()
}

async fn volume(c: &Client) -> f64 {
    let r = call(c, "body_measure", json!({})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    r.structured_content.unwrap()["bodies"][0]["volume_mm3"]
        .as_f64()
        .unwrap()
}

#[tokio::test]
async fn lists_tools_with_object_schemas() {
    let c = connect(ServerOptions::default()).await;
    let tools = c.list_all_tools().await.unwrap();
    let names: Vec<_> = tools.iter().map(|t| t.name.to_string()).collect();
    for n in [
        "solid_box",
        "face_push_pull",
        "edit_undo",
        "edit_redo",
        "body_faces",
    ] {
        assert!(names.contains(&n.to_owned()), "{n} missing in {names:?}");
    }
    for t in &tools {
        assert_eq!(
            t.input_schema.get("type"),
            Some(&json!("object")),
            "{}",
            t.name
        );
        assert!(!t.name.contains('.'));
    }
    // File tools are hidden unless allowed.
    assert!(!names.contains(&"doc_save".to_owned()));
}

#[tokio::test]
async fn box_push_pull_undo_redo() {
    let c = connect(ServerOptions::default()).await;
    let r = call(&c, "solid_box", json!({"size": [100.0, 60.0, 40.0]})).await;
    assert_ne!(r.is_error, Some(true), "{r:?}");
    let faces = call(&c, "body_faces", json!({})).await;
    assert_ne!(faces.is_error, Some(true), "{faces:?}");
    assert!(
        faces
            .structured_content
            .unwrap()
            .to_string()
            .contains("F1:+Z")
    );
    assert_eq!(volume(&c).await, 240_000.0);
    call(
        &c,
        "face_push_pull",
        json!({"face": "F1:+Z", "distance": 10.0}),
    )
    .await;
    assert_eq!(volume(&c).await, 300_000.0);
    call(&c, "edit_undo", json!({})).await;
    assert_eq!(volume(&c).await, 240_000.0);
    call(&c, "edit_redo", json!({})).await;
    assert_eq!(volume(&c).await, 300_000.0);
}

#[tokio::test]
async fn read_only_hides_mutating_tools() {
    let c = connect(ServerOptions {
        read_only: true,
        allow_files: false,
    })
    .await;
    let names: Vec<_> = c
        .list_all_tools()
        .await
        .unwrap()
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    assert!(names.contains(&"body_measure".to_owned()));
    for hidden in ["solid_box", "face_push_pull", "edit_undo"] {
        assert!(!names.contains(&hidden.to_owned()), "{hidden} visible");
    }
    // Hidden tools cannot be called either.
    let err = c
        .call_tool(CallToolRequestParams::new("solid_box".to_owned()))
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn allow_files_exposes_file_tools() {
    let c = connect(ServerOptions {
        read_only: false,
        allow_files: true,
    })
    .await;
    let names: Vec<_> = c
        .list_all_tools()
        .await
        .unwrap()
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    assert!(names.contains(&"doc_save".to_owned()));
}

#[tokio::test]
async fn invalid_calls_are_tool_errors_and_server_survives() {
    let c = connect(ServerOptions::default()).await;
    let bad = call(&c, "solid_box", json!({"size": "huge"})).await;
    assert_eq!(bad.is_error, Some(true));
    let missing = call(
        &c,
        "face_push_pull",
        json!({"face": "F9:+Z", "distance": 1.0}),
    )
    .await;
    assert_eq!(missing.is_error, Some(true));
    assert!(
        c.call_tool(CallToolRequestParams::new("no_such_tool".to_owned()))
            .await
            .is_err()
    );
    // Still serving.
    let ok = call(&c, "solid_box", json!({"size": [10.0, 10.0, 10.0]})).await;
    assert_ne!(ok.is_error, Some(true));
    assert_eq!(volume(&c).await, 1000.0);
}

#[tokio::test]
async fn hostile_arguments_never_crash_the_server() {
    let c = connect(ServerOptions {
        read_only: false,
        allow_files: true,
    })
    .await;
    let huge = "x".repeat(2_000_000);
    let cases = [
        ("solid_box", json!({"size": [1e308, 1e308, 1e308]})),
        ("solid_box", json!({"size": [-1.0, 0.0, f64::MAX]})),
        ("solid_box", json!({"size": null})),
        ("solid_box", json!({"size": [1, 2]})),
        ("solid_box", json!([1, 2, 3])),
        ("face_push_pull", json!({"face": huge, "distance": 1.0})),
        ("face_push_pull", json!({"face": 7, "distance": "x"})),
        ("doc_open", json!({"path": huge})),
        ("selection_set", json!({"faces": [huge]})),
        ("body_measure", json!({"unexpected": {"deep": [[[[]]]]}})),
    ];
    for (tool, args) in cases {
        let r = c
            .call_tool({
                let mut p = CallToolRequestParams::new(tool.to_owned());
                if let Value::Object(m) = args {
                    p = p.with_arguments(m);
                }
                p
            })
            .await;
        assert!(r.is_ok(), "{tool} broke the protocol: {r:?}");
    }
    let ok = call(&c, "solid_box", json!({"size": [10.0, 10.0, 10.0]})).await;
    assert_ne!(ok.is_error, Some(true), "{ok:?}");
}

#[tokio::test]
async fn resources_and_agent_attribution() {
    let c = connect(ServerOptions::default()).await;
    call(&c, "solid_box", json!({"size": [100.0, 60.0, 40.0]})).await;
    let read = |uri: &str| {
        let c = &c;
        let uri = uri.to_owned();
        async move {
            let r = c
                .read_resource(ReadResourceRequestParams::new(uri))
                .await
                .unwrap();
            match r.contents.into_iter().next().unwrap() {
                ResourceContents::TextResourceContents { text, .. } => text,
                other => panic!("unexpected {other:?}"),
            }
        }
    };
    let doc = read("kubit://document").await;
    let doc: Value = serde_json::from_str(&doc).unwrap();
    assert!(doc.to_string().contains("test-agent"), "{doc}");
    assert!(doc.to_string().contains("\"agent\""), "{doc}");
    let model: Value = serde_json::from_str(&read("kubit://model").await).unwrap();
    assert_eq!(model["bodies"][0]["volume_mm3"], json!(240_000.0));
    let names: Vec<_> = c
        .list_resources(None)
        .await
        .unwrap()
        .resources
        .iter()
        .map(|r| r.uri.clone())
        .collect();
    assert_eq!(names, ["kubit://document", "kubit://model"]);
    assert!(
        c.read_resource(ReadResourceRequestParams::new("kubit://nope".to_owned()))
            .await
            .is_err()
    );
}
