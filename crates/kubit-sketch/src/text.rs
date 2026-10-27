//! A small text format for sketches, used by test fixtures and bug reports.
//!
//! One statement per line; `#` starts a comment. Names are free identifiers mapped
//! to entity ids in order of appearance. Lengths are mm, angles degrees.
//!
//! ```text
//! point p1 0 0
//! point p2 30 5
//! line l1 p1 p2
//! circle c1 p3 10          # centre, radius
//! arc a1 p4 p5 p6          # centre, start, end (counter-clockwise)
//! construction l1
//! horizontal l1
//! length l1 30
//! angle l1 l2 90 ref       # `ref` makes a reference dimension
//! ```

use crate::model::{Constraint, ConstraintId, Entity, EntityId, Sketch, SketchError};
use std::collections::BTreeMap;

/// A parse failure, with its 1-based line.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {message}")]
pub struct TextError {
    /// Line number.
    pub line: usize,
    /// What went wrong.
    pub message: String,
}

/// A parsed sketch and the names of its entities and constraints.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parsed {
    /// The sketch.
    pub sketch: Sketch,
    /// Entity names.
    pub names: BTreeMap<String, EntityId>,
    /// Constraint ids in statement order (`constraints[0]` is the first one).
    pub constraints: Vec<ConstraintId>,
}

impl Parsed {
    /// The id named `name`.
    pub fn id(&self, name: &str) -> Option<EntityId> {
        self.names.get(name).copied()
    }
}

/// Parses the text format.
pub fn parse(src: &str) -> Result<Parsed, TextError> {
    let mut out = Parsed::default();
    for (n, raw) in src.lines().enumerate() {
        let line = n + 1;
        let fail = |message: String| TextError { line, message };
        let text = raw.split('#').next().unwrap_or_default().trim();
        if text.is_empty() {
            continue;
        }
        let mut words: Vec<&str> = text.split_whitespace().collect();
        let reference = words.last() == Some(&"ref");
        if reference {
            words.pop();
        }
        let (op, args) = words.split_first().ok_or_else(|| fail("empty".into()))?;
        let id = |i: usize| -> Result<EntityId, TextError> {
            let name = args
                .get(i)
                .ok_or_else(|| fail(format!("`{op}` needs more arguments")))?;
            out.names
                .get(*name)
                .copied()
                .ok_or_else(|| fail(format!("unknown name `{name}`")))
        };
        let num = |i: usize| -> Result<f64, TextError> {
            let w = args
                .get(i)
                .ok_or_else(|| fail(format!("`{op}` needs a number")))?;
            w.parse::<f64>()
                .map_err(|_| fail(format!("`{w}` is not a number")))
        };
        let sketch_err = |e: SketchError| fail(e.to_string());
        let define = |out: &mut Parsed, entity: Entity| -> Result<(), TextError> {
            let name = args.first().ok_or_else(|| fail("missing name".into()))?;
            if out.names.contains_key(*name) {
                return Err(fail(format!("`{name}` defined twice")));
            }
            let id = out.sketch.add_entity(entity, false).map_err(sketch_err)?;
            out.names.insert((*name).to_owned(), id);
            Ok(())
        };
        use Constraint as C;
        let constraint = match *op {
            "point" => {
                let entity = Entity::Point {
                    x: num(1)?,
                    y: num(2)?,
                };
                define(&mut out, entity)?;
                None
            }
            "line" => {
                let entity = Entity::Line {
                    a: id(1)?,
                    b: id(2)?,
                };
                define(&mut out, entity)?;
                None
            }
            "circle" => {
                let entity = Entity::Circle {
                    center: id(1)?,
                    radius: num(2)?,
                };
                define(&mut out, entity)?;
                None
            }
            "arc" => {
                let entity = Entity::Arc {
                    center: id(1)?,
                    start: id(2)?,
                    end: id(3)?,
                };
                define(&mut out, entity)?;
                None
            }
            "construction" => {
                let e = id(0)?;
                if let Some(r) = out.sketch.entities.get_mut(&e) {
                    r.construction = true;
                }
                None
            }
            "coincident" => Some(C::Coincident {
                a: id(0)?,
                b: id(1)?,
            }),
            "horizontal" => Some(C::Horizontal { line: id(0)? }),
            "vertical" => Some(C::Vertical { line: id(0)? }),
            "parallel" => Some(C::Parallel {
                a: id(0)?,
                b: id(1)?,
            }),
            "perpendicular" => Some(C::Perpendicular {
                a: id(0)?,
                b: id(1)?,
            }),
            "tangent" => Some(C::Tangent {
                a: id(0)?,
                b: id(1)?,
            }),
            "equal" => Some(C::Equal {
                a: id(0)?,
                b: id(1)?,
            }),
            "on_line" => Some(C::PointOnLine {
                point: id(0)?,
                line: id(1)?,
            }),
            "on_circle" => Some(C::PointOnCircle {
                point: id(0)?,
                circle: id(1)?,
            }),
            "midpoint" => Some(C::Midpoint {
                point: id(0)?,
                line: id(1)?,
            }),
            "fix" => {
                let p = id(0)?;
                let at = out.sketch.point(p).map_err(sketch_err)?;
                Some(C::Fix { point: p, at })
            }
            "distance" => Some(C::Distance {
                a: id(0)?,
                b: id(1)?,
                value: num(2)?,
            }),
            "hdistance" => Some(C::HorizontalDistance {
                a: id(0)?,
                b: id(1)?,
                value: num(2)?,
            }),
            "vdistance" => Some(C::VerticalDistance {
                a: id(0)?,
                b: id(1)?,
                value: num(2)?,
            }),
            "length" => Some(C::Length {
                line: id(0)?,
                value: num(1)?,
            }),
            "radius" => Some(C::Radius {
                circle: id(0)?,
                value: num(1)?,
            }),
            "diameter" => Some(C::Diameter {
                circle: id(0)?,
                value: num(1)?,
            }),
            "angle" => Some(C::Angle {
                a: id(0)?,
                b: id(1)?,
                value: num(2)?.to_radians(),
            }),
            other => return Err(fail(format!("unknown statement `{other}`"))),
        };
        if let Some(c) = constraint {
            let cid = out
                .sketch
                .add_constraint(c, reference)
                .map_err(sketch_err)?;
            out.constraints.push(cid);
        }
    }
    Ok(out)
}
