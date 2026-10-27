//! The sketch: entities on a plane and the constraints between them.
//!
//! Coordinates are millimetres in the sketch plane (u, v); angles are radians. Ids
//! are handed out in increasing order and never reused, and both maps keep insertion
//! order, so the same edits always give the same sketch (and the same solve).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Largest accepted coordinate or dimension magnitude, mm.
pub const MAX_COORD: f64 = 1e6;

/// Id of an entity of a sketch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(pub u32);

/// Id of a constraint of a sketch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConstraintId(pub u32);

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "e{}", self.0)
    }
}

impl fmt::Display for ConstraintId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "c{}", self.0)
    }
}

/// A sketch entity. Curves reference their points by id, so shared endpoints are
/// shared ids (no coincident constraint needed).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entity {
    /// A point.
    Point {
        /// u, mm.
        x: f64,
        /// v, mm.
        y: f64,
    },
    /// A segment between two points.
    Line {
        /// Start point.
        a: EntityId,
        /// End point.
        b: EntityId,
    },
    /// A full circle.
    Circle {
        /// Centre point.
        center: EntityId,
        /// Radius, mm.
        radius: f64,
    },
    /// A circular arc, counter-clockwise from `start` to `end`. Its radius is the
    /// distance from `center` to `start`; the solver keeps `end` on the same circle.
    Arc {
        /// Centre point.
        center: EntityId,
        /// Start point.
        start: EntityId,
        /// End point.
        end: EntityId,
    },
}

impl Entity {
    /// The points this entity references (not itself).
    pub fn points(&self) -> Vec<EntityId> {
        match *self {
            Self::Point { .. } => Vec::new(),
            Self::Line { a, b } => vec![a, b],
            Self::Circle { center, .. } => vec![center],
            Self::Arc { center, start, end } => vec![center, start, end],
        }
    }

    /// Short kind name, for messages.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Point { .. } => "point",
            Self::Line { .. } => "line",
            Self::Circle { .. } => "circle",
            Self::Arc { .. } => "arc",
        }
    }
}

/// An entity and its flags.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityRecord {
    /// The geometry.
    #[serde(flatten)]
    pub entity: Entity,
    /// Construction geometry: constrains, but is never part of a profile.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub construction: bool,
}

/// A geometric constraint or a dimension.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Constraint {
    /// Two points at the same place.
    Coincident {
        /// First point.
        a: EntityId,
        /// Second point.
        b: EntityId,
    },
    /// A line parallel to the u axis.
    Horizontal {
        /// The line.
        line: EntityId,
    },
    /// A line parallel to the v axis.
    Vertical {
        /// The line.
        line: EntityId,
    },
    /// Two parallel lines.
    Parallel {
        /// First line.
        a: EntityId,
        /// Second line.
        b: EntityId,
    },
    /// Two perpendicular lines.
    Perpendicular {
        /// First line.
        a: EntityId,
        /// Second line.
        b: EntityId,
    },
    /// A line and a circle/arc, or two circles/arcs, touching tangentially.
    Tangent {
        /// First curve.
        a: EntityId,
        /// Second curve.
        b: EntityId,
    },
    /// Two lines of equal length, or two circles/arcs of equal radius.
    Equal {
        /// First curve.
        a: EntityId,
        /// Second curve.
        b: EntityId,
    },
    /// A point on the (infinite) line through a segment.
    PointOnLine {
        /// The point.
        point: EntityId,
        /// The line.
        line: EntityId,
    },
    /// A point on a circle or on an arc's circle.
    PointOnCircle {
        /// The point.
        point: EntityId,
        /// The circle or arc.
        circle: EntityId,
    },
    /// A point at the middle of a segment.
    Midpoint {
        /// The point.
        point: EntityId,
        /// The line.
        line: EntityId,
    },
    /// A point held where it is.
    Fix {
        /// The point.
        point: EntityId,
        /// Where, mm.
        at: [f64; 2],
    },
    /// Distance between two points, mm.
    Distance {
        /// First point.
        a: EntityId,
        /// Second point.
        b: EntityId,
        /// mm.
        value: f64,
    },
    /// Horizontal (u) distance between two points, mm.
    HorizontalDistance {
        /// First point.
        a: EntityId,
        /// Second point.
        b: EntityId,
        /// mm.
        value: f64,
    },
    /// Vertical (v) distance between two points, mm.
    VerticalDistance {
        /// First point.
        a: EntityId,
        /// Second point.
        b: EntityId,
        /// mm.
        value: f64,
    },
    /// Length of a segment, mm.
    Length {
        /// The line.
        line: EntityId,
        /// mm.
        value: f64,
    },
    /// Radius of a circle or arc, mm.
    Radius {
        /// The circle or arc.
        circle: EntityId,
        /// mm.
        value: f64,
    },
    /// Diameter of a circle or arc, mm.
    Diameter {
        /// The circle or arc.
        circle: EntityId,
        /// mm.
        value: f64,
    },
    /// Angle from line `a` to line `b`, counter-clockwise, radians.
    Angle {
        /// First line.
        a: EntityId,
        /// Second line.
        b: EntityId,
        /// Radians.
        value: f64,
    },
}

impl Constraint {
    /// The entities it references.
    pub fn entities(&self) -> Vec<EntityId> {
        use Constraint as C;
        match *self {
            C::Horizontal { line } | C::Vertical { line } | C::Length { line, .. } => vec![line],
            C::Radius { circle, .. } | C::Diameter { circle, .. } => vec![circle],
            C::Fix { point, .. } => vec![point],
            C::PointOnLine { point, line } | C::Midpoint { point, line } => vec![point, line],
            C::PointOnCircle { point, circle } => vec![point, circle],
            C::Coincident { a, b }
            | C::Parallel { a, b }
            | C::Perpendicular { a, b }
            | C::Tangent { a, b }
            | C::Equal { a, b }
            | C::Distance { a, b, .. }
            | C::HorizontalDistance { a, b, .. }
            | C::VerticalDistance { a, b, .. }
            | C::Angle { a, b, .. } => vec![a, b],
        }
    }

    /// The dimension value, for dimensions.
    pub fn value(&self) -> Option<f64> {
        use Constraint as C;
        match *self {
            C::Distance { value, .. }
            | C::HorizontalDistance { value, .. }
            | C::VerticalDistance { value, .. }
            | C::Length { value, .. }
            | C::Radius { value, .. }
            | C::Diameter { value, .. }
            | C::Angle { value, .. } => Some(value),
            _ => None,
        }
    }

    /// Replaces the dimension value; `false` for non-dimensions.
    pub fn set_value(&mut self, v: f64) -> bool {
        use Constraint as C;
        match self {
            C::Distance { value, .. }
            | C::HorizontalDistance { value, .. }
            | C::VerticalDistance { value, .. }
            | C::Length { value, .. }
            | C::Radius { value, .. }
            | C::Diameter { value, .. }
            | C::Angle { value, .. } => {
                *value = v;
                true
            }
            _ => false,
        }
    }

    /// Short kind name, for messages.
    pub fn kind(&self) -> &'static str {
        use Constraint as C;
        match self {
            C::Coincident { .. } => "coincident",
            C::Horizontal { .. } => "horizontal",
            C::Vertical { .. } => "vertical",
            C::Parallel { .. } => "parallel",
            C::Perpendicular { .. } => "perpendicular",
            C::Tangent { .. } => "tangent",
            C::Equal { .. } => "equal",
            C::PointOnLine { .. } => "point_on_line",
            C::PointOnCircle { .. } => "point_on_circle",
            C::Midpoint { .. } => "midpoint",
            C::Fix { .. } => "fix",
            C::Distance { .. } => "distance",
            C::HorizontalDistance { .. } => "horizontal_distance",
            C::VerticalDistance { .. } => "vertical_distance",
            C::Length { .. } => "length",
            C::Radius { .. } => "radius",
            C::Diameter { .. } => "diameter",
            C::Angle { .. } => "angle",
        }
    }
}

/// A constraint and its flags.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConstraintRecord {
    /// The constraint.
    #[serde(flatten)]
    pub constraint: Constraint,
    /// A reference (driven) dimension: shown and measured, never enforced.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reference: bool,
}

/// Why an edit was refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SketchError {
    /// The id does not exist.
    #[error("unknown entity {0}")]
    UnknownEntity(EntityId),
    /// The id does not exist.
    #[error("unknown constraint {0}")]
    UnknownConstraint(ConstraintId),
    /// The entity is of the wrong kind for this use.
    #[error("{id} is a {found}, expected {expected}")]
    WrongKind {
        /// The entity.
        id: EntityId,
        /// What it is.
        found: &'static str,
        /// What was needed.
        expected: &'static str,
    },
    /// A number is not finite or out of bounds.
    #[error("{0} must be finite and within ±1e6")]
    BadNumber(&'static str),
    /// A constraint references the same entity twice where two are needed.
    #[error("{0} needs two different entities")]
    SameEntity(&'static str),
    /// The entity is still referenced.
    #[error("{0} is used by {1}")]
    InUse(EntityId, String),
    /// The sketch has run out of ids.
    #[error("too many entities")]
    Full,
}

/// A 2D sketch.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sketch {
    /// Entities, in insertion order.
    pub entities: IndexMap<EntityId, EntityRecord>,
    /// Constraints, in insertion order.
    pub constraints: IndexMap<ConstraintId, ConstraintRecord>,
    /// Next id to hand out (shared by entities and constraints).
    #[serde(default)]
    pub next_id: u32,
}

fn bounded(v: f64, what: &'static str) -> Result<f64, SketchError> {
    if v.is_finite() && v.abs() <= MAX_COORD {
        Ok(v)
    } else {
        Err(SketchError::BadNumber(what))
    }
}

impl Sketch {
    /// An empty sketch.
    pub fn new() -> Self {
        Self::default()
    }

    fn next(&mut self) -> Result<u32, SketchError> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or(SketchError::Full)?;
        Ok(id)
    }

    /// The entity `id`.
    pub fn entity(&self, id: EntityId) -> Result<&Entity, SketchError> {
        self.entities
            .get(&id)
            .map(|r| &r.entity)
            .ok_or(SketchError::UnknownEntity(id))
    }

    fn expect(
        &self,
        id: EntityId,
        expected: &'static str,
        ok: fn(&Entity) -> bool,
    ) -> Result<(), SketchError> {
        let e = self.entity(id)?;
        if ok(e) {
            Ok(())
        } else {
            Err(SketchError::WrongKind {
                id,
                found: e.kind(),
                expected,
            })
        }
    }

    fn expect_point(&self, id: EntityId) -> Result<(), SketchError> {
        self.expect(id, "point", |e| matches!(e, Entity::Point { .. }))
    }

    fn expect_line(&self, id: EntityId) -> Result<(), SketchError> {
        self.expect(id, "line", |e| matches!(e, Entity::Line { .. }))
    }

    fn expect_round(&self, id: EntityId) -> Result<(), SketchError> {
        self.expect(id, "circle or arc", |e| {
            matches!(e, Entity::Circle { .. } | Entity::Arc { .. })
        })
    }

    fn expect_curve(&self, id: EntityId) -> Result<(), SketchError> {
        self.expect(id, "line, circle or arc", |e| {
            !matches!(e, Entity::Point { .. })
        })
    }

    /// Adds an entity after checking its references and numbers.
    pub fn add_entity(
        &mut self,
        entity: Entity,
        construction: bool,
    ) -> Result<EntityId, SketchError> {
        match entity {
            Entity::Point { x, y } => {
                bounded(x, "x")?;
                bounded(y, "y")?;
            }
            Entity::Line { a, b } => {
                self.expect_point(a)?;
                self.expect_point(b)?;
                if a == b {
                    return Err(SketchError::SameEntity("line"));
                }
            }
            Entity::Circle { center, radius } => {
                self.expect_point(center)?;
                if bounded(radius, "radius")? <= 0.0 {
                    return Err(SketchError::BadNumber("radius"));
                }
            }
            Entity::Arc { center, start, end } => {
                for p in [center, start, end] {
                    self.expect_point(p)?;
                }
                if center == start || center == end || start == end {
                    return Err(SketchError::SameEntity("arc"));
                }
            }
        }
        let id = EntityId(self.next()?);
        self.entities.insert(
            id,
            EntityRecord {
                entity,
                construction,
            },
        );
        Ok(id)
    }

    /// Adds a point.
    pub fn add_point(&mut self, x: f64, y: f64) -> Result<EntityId, SketchError> {
        self.add_entity(Entity::Point { x, y }, false)
    }

    /// Adds a segment between two points.
    pub fn add_line(&mut self, a: EntityId, b: EntityId) -> Result<EntityId, SketchError> {
        self.add_entity(Entity::Line { a, b }, false)
    }

    /// Adds a constraint after checking that it references the right kinds.
    pub fn add_constraint(
        &mut self,
        constraint: Constraint,
        reference: bool,
    ) -> Result<ConstraintId, SketchError> {
        use Constraint as C;
        let two = |a: EntityId, b: EntityId, what| {
            if a == b {
                Err(SketchError::SameEntity(what))
            } else {
                Ok(())
            }
        };
        match constraint {
            C::Coincident { a, b }
            | C::Distance { a, b, .. }
            | C::HorizontalDistance { a, b, .. }
            | C::VerticalDistance { a, b, .. } => {
                self.expect_point(a)?;
                self.expect_point(b)?;
                two(a, b, constraint.kind())?;
            }
            C::Horizontal { line } | C::Vertical { line } | C::Length { line, .. } => {
                self.expect_line(line)?;
            }
            C::Parallel { a, b } | C::Perpendicular { a, b } | C::Angle { a, b, .. } => {
                self.expect_line(a)?;
                self.expect_line(b)?;
                two(a, b, constraint.kind())?;
            }
            C::Tangent { a, b } => {
                self.expect_curve(a)?;
                self.expect_curve(b)?;
                two(a, b, "tangent")?;
                let lines = [a, b]
                    .iter()
                    .filter(|id| matches!(self.entity(**id), Ok(Entity::Line { .. })))
                    .count();
                if lines == 2 {
                    return Err(SketchError::WrongKind {
                        id: b,
                        found: "line",
                        expected: "circle or arc",
                    });
                }
            }
            C::Equal { a, b } => {
                self.expect_curve(a)?;
                self.expect_curve(b)?;
                two(a, b, "equal")?;
                let line = |id| matches!(self.entity(id), Ok(Entity::Line { .. }));
                if line(a) != line(b) {
                    return Err(SketchError::WrongKind {
                        id: b,
                        found: self.entity(b)?.kind(),
                        expected: self.entity(a)?.kind(),
                    });
                }
            }
            C::PointOnLine { point, line } | C::Midpoint { point, line } => {
                self.expect_point(point)?;
                self.expect_line(line)?;
            }
            C::PointOnCircle { point, circle } => {
                self.expect_point(point)?;
                self.expect_round(circle)?;
            }
            C::Radius { circle, .. } | C::Diameter { circle, .. } => self.expect_round(circle)?,
            C::Fix { point, at } => {
                self.expect_point(point)?;
                bounded(at[0], "fix u")?;
                bounded(at[1], "fix v")?;
            }
        }
        if let Some(v) = constraint.value() {
            bounded(v, "dimension")?;
            let length_like = !matches!(
                constraint,
                C::Angle { .. } | C::HorizontalDistance { .. } | C::VerticalDistance { .. }
            );
            if length_like && v < 0.0 {
                return Err(SketchError::BadNumber("dimension"));
            }
            if matches!(constraint, C::Radius { .. } | C::Diameter { .. }) && v <= 0.0 {
                return Err(SketchError::BadNumber("dimension"));
            }
        }
        let id = ConstraintId(self.next()?);
        self.constraints.insert(
            id,
            ConstraintRecord {
                constraint,
                reference,
            },
        );
        Ok(id)
    }

    /// Changes a dimension's value.
    pub fn set_dimension(&mut self, id: ConstraintId, value: f64) -> Result<(), SketchError> {
        let mut c = self
            .constraints
            .get(&id)
            .copied()
            .ok_or(SketchError::UnknownConstraint(id))?;
        let reference = c.reference;
        if !c.constraint.set_value(value) {
            return Err(SketchError::BadNumber("not a dimension"));
        }
        // Re-validate through a scratch copy so the rules live in one place.
        let mut scratch = self.clone();
        scratch.constraints.shift_remove(&id);
        scratch.add_constraint(c.constraint, reference)?;
        if let Some(slot) = self.constraints.get_mut(&id) {
            slot.constraint = c.constraint;
        }
        Ok(())
    }

    /// Removes a constraint.
    pub fn remove_constraint(&mut self, id: ConstraintId) -> Result<(), SketchError> {
        self.constraints
            .shift_remove(&id)
            .map(|_| ())
            .ok_or(SketchError::UnknownConstraint(id))
    }

    /// Removes an entity, the constraints on it, and the points only it used.
    pub fn remove_entity(&mut self, id: EntityId) -> Result<(), SketchError> {
        let entity = *self.entity(id)?;
        if let Some((user, _)) = self
            .entities
            .iter()
            .find(|(other, r)| **other != id && r.entity.points().contains(&id))
        {
            return Err(SketchError::InUse(id, user.to_string()));
        }
        self.entities.shift_remove(&id);
        self.constraints
            .retain(|_, c| !c.constraint.entities().contains(&id));
        for p in entity.points() {
            let used = self
                .entities
                .values()
                .any(|r| r.entity.points().contains(&p))
                || self
                    .constraints
                    .values()
                    .any(|c| c.constraint.entities().contains(&p));
            if !used {
                self.entities.shift_remove(&p);
            }
        }
        Ok(())
    }

    /// Position of point `id`.
    pub fn point(&self, id: EntityId) -> Result<[f64; 2], SketchError> {
        match self.entity(id)? {
            Entity::Point { x, y } => Ok([*x, *y]),
            other => Err(SketchError::WrongKind {
                id,
                found: other.kind(),
                expected: "point",
            }),
        }
    }

    /// Moves point `id` without solving.
    pub fn set_point(&mut self, id: EntityId, at: [f64; 2]) -> Result<(), SketchError> {
        bounded(at[0], "x")?;
        bounded(at[1], "y")?;
        match self.entities.get_mut(&id).map(|r| &mut r.entity) {
            Some(Entity::Point { x, y }) => {
                *x = at[0];
                *y = at[1];
                Ok(())
            }
            Some(other) => Err(SketchError::WrongKind {
                id,
                found: other.kind(),
                expected: "point",
            }),
            None => Err(SketchError::UnknownEntity(id)),
        }
    }

    /// Radius of a circle or arc.
    pub fn radius(&self, id: EntityId) -> Result<f64, SketchError> {
        match *self.entity(id)? {
            Entity::Circle { radius, .. } => Ok(radius),
            Entity::Arc { center, start, .. } => {
                let (c, s) = (self.point(center)?, self.point(start)?);
                Ok(((s[0] - c[0]).powi(2) + (s[1] - c[1]).powi(2)).sqrt())
            }
            ref other => Err(SketchError::WrongKind {
                id,
                found: other.kind(),
                expected: "circle or arc",
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_increasing_and_never_reused() {
        let mut s = Sketch::new();
        let a = s.add_point(0.0, 0.0).unwrap();
        let b = s.add_point(1.0, 0.0).unwrap();
        let l = s.add_line(a, b).unwrap();
        assert_eq!((a, b, l), (EntityId(0), EntityId(1), EntityId(2)));
        s.remove_entity(l).unwrap();
        assert!(s.entities.is_empty(), "orphan endpoints go with the line");
        assert_eq!(s.add_point(0.0, 0.0).unwrap(), EntityId(3));
    }

    #[test]
    fn constraints_check_kinds_and_numbers() {
        let mut s = Sketch::new();
        let a = s.add_point(0.0, 0.0).unwrap();
        let b = s.add_point(1.0, 0.0).unwrap();
        let l = s.add_line(a, b).unwrap();
        assert!(matches!(
            s.add_constraint(Constraint::Horizontal { line: a }, false),
            Err(SketchError::WrongKind { .. })
        ));
        assert!(
            s.add_constraint(
                Constraint::Length {
                    line: l,
                    value: f64::NAN
                },
                false
            )
            .is_err()
        );
        assert!(
            s.add_constraint(
                Constraint::Length {
                    line: l,
                    value: -1.0
                },
                false
            )
            .is_err()
        );
        assert!(
            s.add_constraint(Constraint::Coincident { a, b: a }, false)
                .is_err()
        );
        let c = s
            .add_constraint(
                Constraint::Length {
                    line: l,
                    value: 5.0,
                },
                false,
            )
            .unwrap();
        s.set_dimension(c, 7.0).unwrap();
        assert_eq!(
            s.constraints.get(&c).and_then(|r| r.constraint.value()),
            Some(7.0)
        );
        assert!(s.set_dimension(c, f64::INFINITY).is_err());
        assert!(s.remove_entity(a).is_err(), "the line still uses it");
    }

    #[test]
    fn serde_round_trip_keeps_order() {
        let mut s = Sketch::new();
        let a = s.add_point(0.0, 0.0).unwrap();
        let b = s.add_point(3.0, 4.0).unwrap();
        let l = s.add_entity(Entity::Line { a, b }, true).unwrap();
        s.add_constraint(
            Constraint::Length {
                line: l,
                value: 5.0,
            },
            true,
        )
        .unwrap();
        let json = serde_json::to_string(&s).unwrap();
        let back: Sketch = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        assert!(json.contains(r#""construction":true"#));
    }
}
