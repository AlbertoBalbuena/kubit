//! The `.kubit` v1 container and its JSON form (`docs/ARCHITECTURE.md` §9).
//!
//! A `.kubit` file is a ZIP with stored (uncompressed) entries and fixed
//! timestamps, so the same document always produces the same bytes:
//!
//! - `manifest.json` — container format and version, app version;
//! - `document.json` — the parametric document (sorted keys, pretty-printed);
//! - `catalog-snapshot.json` — pinned catalog entries (empty until Phase 2);
//! - `thumbnail.png` — optional.
//!
//! `cache/` (B-rep and meshes) is reserved; v1 writes none and ignores it on read.

use crate::document::{Document, DocumentError, FORMAT_VERSION};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

/// Container identifier in `manifest.json`.
pub const CONTAINER: &str = "kubit";
/// Largest entry we read, to refuse zip bombs and hostile files (64 MiB).
pub const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
/// Most entries we accept in a container.
pub const MAX_ENTRIES: usize = 4096;

/// `manifest.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Always `"kubit"`.
    pub container: String,
    /// Version of `document.json` inside.
    pub format_version: u32,
    /// Version of the app that wrote the file.
    pub app_version: String,
}

/// Errors reading or writing documents.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    /// Not a ZIP, or a broken one.
    #[error("not a valid .kubit container: {0}")]
    Container(String),
    /// A required entry is missing.
    #[error("missing `{0}` in the .kubit container")]
    MissingEntry(&'static str),
    /// An entry is larger than [`MAX_ENTRY_BYTES`] or there are too many entries.
    #[error("the file exceeds the safety limits: {0}")]
    TooLarge(String),
    /// JSON syntax or schema error.
    #[error("invalid JSON in `{entry}`: {message}")]
    Json {
        /// Entry name.
        entry: &'static str,
        /// serde_json message (with line and column).
        message: String,
    },
    /// The manifest is not a Kubit manifest.
    #[error("not a Kubit file (container `{0}`)")]
    NotKubit(String),
    /// The document breaks an invariant.
    #[error(transparent)]
    Document(#[from] DocumentError),
}

fn options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644)
}

/// `document.json` text: pretty, sorted object keys, trailing newline.
pub fn to_json(doc: &Document) -> Result<String, FormatError> {
    let mut s = serde_json::to_string_pretty(doc).map_err(|e| FormatError::Json {
        entry: "document.json",
        message: e.to_string(),
    })?;
    s.push('\n');
    Ok(s)
}

/// Parses and validates `document.json` text (the "Save as JSON" form too).
pub fn from_json(text: &str) -> Result<Document, FormatError> {
    if text.len() as u64 > MAX_ENTRY_BYTES {
        return Err(FormatError::TooLarge("document.json".into()));
    }
    // Peek at the version first; the document itself is parsed straight from the
    // text because `serde_json::Value` sorts keys and would lose the parameter order.
    #[derive(Deserialize)]
    struct Version {
        #[serde(default)]
        format_version: u32,
    }
    let version: Version = serde_json::from_str(text).map_err(|e| FormatError::Json {
        entry: "document.json",
        message: e.to_string(),
    })?;
    check_version(version.format_version)?;
    let doc: Document = serde_json::from_str(text).map_err(|e| FormatError::Json {
        entry: "document.json",
        message: e.to_string(),
    })?;
    doc.validate()?;
    Ok(doc)
}

/// Rejects future versions. v1 is the first format; each later version adds a
/// migration step here (on an order-preserving representation) plus a fixture test.
fn check_version(version: u32) -> Result<(), FormatError> {
    if version > FORMAT_VERSION {
        return Err(DocumentError::TooNew {
            found: version,
            supported: FORMAT_VERSION,
        }
        .into());
    }
    Ok(())
}

fn zip_err(e: impl std::fmt::Display) -> FormatError {
    FormatError::Container(e.to_string())
}

/// Writes a `.kubit` container.
pub fn to_kubit(doc: &Document, thumbnail_png: Option<&[u8]>) -> Result<Vec<u8>, FormatError> {
    let manifest = Manifest {
        container: CONTAINER.into(),
        format_version: FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
    };
    let manifest = serde_json::to_string_pretty(&manifest).map_err(|e| FormatError::Json {
        entry: "manifest.json",
        message: e.to_string(),
    })?;
    let doc_json = to_json(doc)?;
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let entries: [(&str, Option<&[u8]>); 4] = [
        ("manifest.json", Some(manifest.as_bytes())),
        ("document.json", Some(doc_json.as_bytes())),
        ("catalog-snapshot.json", Some(b"{}\n")),
        ("thumbnail.png", thumbnail_png),
    ];
    for (name, data) in entries {
        if let Some(bytes) = data {
            zip.start_file(name, options()).map_err(zip_err)?;
            zip.write_all(bytes).map_err(zip_err)?;
        }
    }
    Ok(zip.finish().map_err(zip_err)?.into_inner())
}

fn read_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &'static str,
) -> Result<Option<String>, FormatError> {
    let mut file = match archive.by_name(name) {
        Ok(f) => f,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(zip_err(e)),
    };
    if file.size() > MAX_ENTRY_BYTES {
        return Err(FormatError::TooLarge(name.into()));
    }
    let mut text = String::new();
    // `take` bounds the read even if the header lies about the size.
    file.by_ref()
        .take(MAX_ENTRY_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|e| FormatError::Json {
            entry: name,
            message: e.to_string(),
        })?;
    if text.len() as u64 > MAX_ENTRY_BYTES {
        return Err(FormatError::TooLarge(name.into()));
    }
    Ok(Some(text))
}

/// Reads a `.kubit` container.
pub fn from_kubit(bytes: &[u8]) -> Result<Document, FormatError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(zip_err)?;
    if archive.len() > MAX_ENTRIES {
        return Err(FormatError::TooLarge(format!("{} entries", archive.len())));
    }
    let manifest = read_entry(&mut archive, "manifest.json")?
        .ok_or(FormatError::MissingEntry("manifest.json"))?;
    let manifest: Manifest = serde_json::from_str(&manifest).map_err(|e| FormatError::Json {
        entry: "manifest.json",
        message: e.to_string(),
    })?;
    if manifest.container != CONTAINER {
        return Err(FormatError::NotKubit(manifest.container));
    }
    let text = read_entry(&mut archive, "document.json")?
        .ok_or(FormatError::MissingEntry("document.json"))?;
    from_json(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Author, Parameter};
    use proptest::prelude::*;

    fn sample() -> Document {
        let mut d = Document::new();
        d.parameters.insert(
            "ancho".into(),
            Parameter {
                expression: "600 mm".into(),
                comment: String::new(),
            },
        );
        d.parameters.insert(
            "alto".into(),
            Parameter {
                expression: "ancho / 2".into(),
                comment: "half".into(),
            },
        );
        d.push_feature(
            "solid.box",
            serde_json::json!({"origin": [0.0, 0.0, 0.0], "size": [100.0, 60.0, 40.0]}),
            Author::User,
        );
        d.push_feature(
            "face.push_pull",
            serde_json::json!({"face": "F1:+Z", "distance": 10.0}),
            Author::Agent {
                name: "claude".into(),
            },
        );
        d
    }

    #[test]
    fn kubit_round_trip_is_identical() {
        let doc = sample();
        let bytes = to_kubit(&doc, None).unwrap();
        let back = from_kubit(&bytes).unwrap();
        assert_eq!(back, doc);
        // `IndexMap` equality ignores order, so also compare the serialized forms.
        assert_eq!(to_json(&back).unwrap(), to_json(&doc).unwrap());
        // Deterministic container: saving again gives the same bytes.
        assert!(
            to_kubit(&back, None).unwrap() == bytes,
            "re-saved container differs"
        );
    }

    #[test]
    fn json_round_trip_and_key_order() {
        let doc = sample();
        let text = to_json(&doc).unwrap();
        assert_eq!(from_json(&text).unwrap(), doc);
        assert!(
            text.find("\"ancho\"") < text.find("\"alto\""),
            "parameters keep creation order"
        );
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn thumbnail_is_optional() {
        let with = to_kubit(&sample(), Some(&[0x89, b'P', b'N', b'G'])).unwrap();
        assert_eq!(from_kubit(&with).unwrap(), sample());
    }

    #[test]
    fn rejects_foreign_and_future_files() {
        assert!(matches!(
            from_kubit(b"not a zip"),
            Err(FormatError::Container(_))
        ));
        let mut future = serde_json::to_value(sample()).unwrap();
        future["format_version"] = serde_json::json!(2);
        assert!(matches!(
            from_json(&future.to_string()),
            Err(FormatError::Document(DocumentError::TooNew {
                found: 2,
                ..
            }))
        ));
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("manifest.json", options()).unwrap();
        zip.write_all(br#"{"container":"other","format_version":1,"app_version":"x"}"#)
            .unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert!(matches!(from_kubit(&bytes), Err(FormatError::NotKubit(_))));
    }

    proptest! {
        #[test]
        fn hostile_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
            let _ = from_kubit(&bytes);
            let _ = from_json(&String::from_utf8_lossy(&bytes));
        }

        #[test]
        fn mutated_containers_never_panic(i in 0usize..4096, b in any::<u8>()) {
            let mut bytes = to_kubit(&sample(), None).unwrap();
            let at = i % bytes.len().max(1);
            if let Some(slot) = bytes.get_mut(at) {
                *slot = b;
            }
            let _ = from_kubit(&bytes);
        }
    }
}
