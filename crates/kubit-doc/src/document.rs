//! The document: parameters and the timeline of features. Geometry is derived by
//! replaying the timeline; it is never the source of truth.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Version of `document.json` written by this build.
pub const FORMAT_VERSION: u32 = 1;

/// Id of a feature in the timeline, shown as `F<n>`. Never reused in a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FeatureId(pub u32);

impl fmt::Display for FeatureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "F{}", self.0)
    }
}

impl FromStr for FeatureId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix('F')
            .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|n| n.parse().ok())
            .map(FeatureId)
            .ok_or_else(|| {
                format!(
                    "invalid feature id `{}`",
                    s.chars().take(32).collect::<String>()
                )
            })
    }
}

impl TryFrom<String> for FeatureId {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<FeatureId> for String {
    fn from(id: FeatureId) -> String {
        id.to_string()
    }
}

/// Who created a feature: the timeline shows agent actions distinctly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Author {
    /// A person, through the UI or the CLI.
    User,
    /// An AI agent through MCP.
    Agent {
        /// Client name reported by the MCP client.
        name: String,
    },
    /// A script or batch file.
    Script {
        /// Script path or name.
        source: String,
    },
}

/// One step of the timeline: a registered command with its parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    /// Stable id.
    pub id: FeatureId,
    /// Command id, e.g. `solid.box`.
    pub command: String,
    /// Canonical parameters (validated by the command's schema).
    pub params: serde_json::Value,
    /// Who created it.
    pub author: Author,
    /// Suppressed features are skipped on rebuild.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub suppressed: bool,
}

/// A named parameter. The expression is always stored, never only its value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    /// Expression text, e.g. `ancho / 2 + 3 mm`.
    pub expression: String,
    /// Optional note.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
}

/// How lengths are shown (internal values are always mm).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LengthDisplay {
    /// Millimetres.
    #[default]
    Millimetre,
    /// Inches with fractions.
    Inch,
}

/// Document-wide tolerances.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tolerances {
    /// Linear, mm.
    pub linear_mm: f64,
    /// Angular, radians.
    pub angular_rad: f64,
}

impl Default for Tolerances {
    fn default() -> Self {
        Self {
            linear_mm: 1e-3,
            angular_rad: 1e-6,
        }
    }
}

/// The parametric document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// `document.json` format version.
    pub format_version: u32,
    /// Display units.
    #[serde(default)]
    pub length_display: LengthDisplay,
    /// Tolerances.
    #[serde(default)]
    pub tolerances: Tolerances,
    /// Parameters in creation order.
    #[serde(default)]
    pub parameters: IndexMap<String, Parameter>,
    /// The timeline.
    #[serde(default)]
    pub features: Vec<Feature>,
    /// Next id to hand out; ids are never reused, even after deleting a feature.
    pub next_feature: u32,
    /// Rollback marker: the timeline is applied up to and including this feature;
    /// later ones are rolled back. `None` means the end of the timeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback: Option<FeatureId>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            length_display: LengthDisplay::default(),
            tolerances: Tolerances::default(),
            parameters: IndexMap::new(),
            features: Vec::new(),
            next_feature: 1,
            rollback: None,
        }
    }
}

/// Structural problems found by [`Document::validate`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DocumentError {
    /// Unknown future format.
    #[error("document format {found} is newer than this build supports ({supported})")]
    TooNew {
        /// Version in the file.
        found: u32,
        /// Version this build writes.
        supported: u32,
    },
    /// Two features share an id, or an id is not below `next_feature`.
    #[error("feature ids must be unique; offending id {0}")]
    BadFeatureOrder(FeatureId),
    /// The rollback marker names a feature that does not exist.
    #[error("rollback marker {0} is not a feature of the document")]
    BadRollback(FeatureId),
    /// `next_feature` would reuse an existing id.
    #[error("next_feature {0} is not above the last feature id")]
    BadNextFeature(u32),
    /// A number in the parameters is not finite.
    #[error("feature {0} has a non-finite number in its parameters")]
    NonFinite(FeatureId),
}

fn all_finite(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Number(n) => n.as_f64().is_some_and(f64::is_finite),
        serde_json::Value::Array(a) => a.iter().all(all_finite),
        serde_json::Value::Object(o) => o.values().all(all_finite),
        _ => true,
    }
}

impl Document {
    /// An empty document.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserves the next feature id.
    pub fn allocate_feature_id(&mut self) -> FeatureId {
        let id = FeatureId(self.next_feature);
        self.next_feature = self.next_feature.saturating_add(1);
        id
    }

    /// Inserts a feature with a fresh id right after the rollback marker (at the
    /// end when there is none), moves the marker onto it, and returns the id.
    pub fn push_feature(
        &mut self,
        command: &str,
        params: serde_json::Value,
        author: Author,
    ) -> FeatureId {
        let id = self.allocate_feature_id();
        let feature = Feature {
            id,
            command: command.to_owned(),
            params,
            author,
            suppressed: false,
        };
        let at = self
            .rollback
            .and_then(|m| self.features.iter().position(|f| f.id == m))
            .map_or(self.features.len(), |i| i + 1);
        self.features.insert(at, feature);
        if self.rollback.is_some() {
            self.rollback = Some(id);
        }
        id
    }

    /// The feature with `id`.
    pub fn feature(&self, id: FeatureId) -> Option<&Feature> {
        self.features.iter().find(|f| f.id == id)
    }

    /// Checks the invariants every loaded document must satisfy.
    pub fn validate(&self) -> Result<(), DocumentError> {
        if self.format_version > FORMAT_VERSION {
            return Err(DocumentError::TooNew {
                found: self.format_version,
                supported: FORMAT_VERSION,
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut last = 0;
        for f in &self.features {
            // Ids are unique but not sorted: a feature added while the timeline is
            // rolled back sits in the middle with a fresh, larger id.
            if !seen.insert(f.id) {
                return Err(DocumentError::BadFeatureOrder(f.id));
            }
            last = last.max(f.id.0);
            if !all_finite(&f.params) {
                return Err(DocumentError::NonFinite(f.id));
            }
        }
        if self.next_feature <= last {
            return Err(DocumentError::BadNextFeature(self.next_feature));
        }
        if let Some(m) = self.rollback.filter(|m| !seen.contains(m)) {
            return Err(DocumentError::BadRollback(m));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_increasing_and_never_reused() {
        let mut d = Document::new();
        let a = d.push_feature("solid.box", serde_json::json!({}), Author::User);
        let b = d.push_feature("face.push_pull", serde_json::json!({}), Author::User);
        assert_eq!((a, b), (FeatureId(1), FeatureId(2)));
        d.features.pop();
        assert_eq!(d.allocate_feature_id(), FeatureId(3));
        assert!(d.validate().is_ok());
    }

    #[test]
    fn validation_catches_bad_documents() {
        let mut d = Document::new();
        d.push_feature("solid.box", serde_json::json!({}), Author::User);
        let mut dup = d.clone();
        dup.features.push(dup.features[0].clone());
        assert!(matches!(
            dup.validate(),
            Err(DocumentError::BadFeatureOrder(_))
        ));
        let mut next = d.clone();
        next.next_feature = 1;
        assert!(matches!(
            next.validate(),
            Err(DocumentError::BadNextFeature(1))
        ));
        let mut new = d;
        new.format_version = 99;
        assert!(matches!(new.validate(), Err(DocumentError::TooNew { .. })));
    }

    #[test]
    fn push_inserts_after_the_marker_and_moves_it() {
        let mut d = Document::new();
        let a = d.push_feature("solid.box", serde_json::json!({}), Author::User);
        let b = d.push_feature("face.push_pull", serde_json::json!({}), Author::User);
        d.rollback = Some(a);
        let c = d.push_feature("edge.fillet", serde_json::json!({}), Author::User);
        let order: Vec<_> = d.features.iter().map(|f| f.id).collect();
        assert_eq!(order, [a, c, b]);
        assert_eq!(d.rollback, Some(c));
        assert!(d.validate().is_ok());
        d.rollback = Some(FeatureId(99));
        assert!(matches!(d.validate(), Err(DocumentError::BadRollback(_))));
    }

    #[test]
    fn feature_id_text_form() {
        assert_eq!("F7".parse::<FeatureId>(), Ok(FeatureId(7)));
        assert!("7".parse::<FeatureId>().is_err());
        assert!("F".parse::<FeatureId>().is_err());
        assert!("F+1".parse::<FeatureId>().is_err());
    }
}
