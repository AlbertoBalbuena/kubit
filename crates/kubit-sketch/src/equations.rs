//! Residual equations: what each constraint asks of the variables.
//!
//! The unknowns are every point's (u, v) and every circle's radius, laid out in
//! entity order ([`Layout`]). Each [`Equation`] yields one or two residual rows with
//! their exact gradient (forward-mode duals). Residuals are scaled to millimetres so
//! one tolerance fits all of them; angle residuals are multiplied by the mean length
//! of the two lines for the same reason.

use crate::dual::{Dual, MAX_LOCAL, V2};
use crate::model::{Constraint, ConstraintId, Entity, EntityId, Sketch};
use std::collections::BTreeMap;

/// Where each entity's unknowns sit in the variable vector.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    /// First of the two variables of each point.
    pub point: BTreeMap<EntityId, usize>,
    /// The radius variable of each circle (arcs derive theirs from their points).
    pub radius: BTreeMap<EntityId, usize>,
    /// Number of variables.
    pub len: usize,
}

impl Layout {
    /// Lays out `sketch` and returns the layout and the current values.
    pub fn of(sketch: &Sketch) -> (Self, Vec<f64>) {
        let mut layout = Self::default();
        let mut x = Vec::new();
        for (id, r) in &sketch.entities {
            match r.entity {
                Entity::Point { x: u, y: v } => {
                    layout.point.insert(*id, x.len());
                    x.extend([u, v]);
                }
                Entity::Circle { radius, .. } => {
                    layout.radius.insert(*id, x.len());
                    x.push(radius);
                }
                Entity::Line { .. } | Entity::Arc { .. } => {}
            }
        }
        layout.len = x.len();
        (layout, x)
    }

    /// Writes `x` back into the sketch.
    pub fn store(&self, sketch: &mut Sketch, x: &[f64]) {
        for (id, r) in sketch.entities.iter_mut() {
            match &mut r.entity {
                Entity::Point { x: u, y: v } => {
                    if let Some(&i) = self.point.get(id)
                        && let (Some(a), Some(b)) = (x.get(i), x.get(i + 1))
                    {
                        *u = *a;
                        *v = *b;
                    }
                }
                Entity::Circle { radius, .. } => {
                    if let Some(&i) = self.radius.get(id)
                        && let Some(r) = x.get(i)
                    {
                        *radius = *r;
                    }
                }
                Entity::Line { .. } | Entity::Arc { .. } => {}
            }
        }
    }

    /// The variables of an entity (a curve's include its points').
    pub fn vars_of(&self, sketch: &Sketch, id: EntityId) -> Vec<usize> {
        let mut out = Vec::new();
        if let Some(&i) = self.point.get(&id) {
            out.extend([i, i + 1]);
        }
        if let Some(&i) = self.radius.get(&id) {
            out.push(i);
        }
        if let Ok(e) = sketch.entity(id) {
            for p in e.points() {
                if let Some(&i) = self.point.get(&p) {
                    out.extend([i, i + 1]);
                }
            }
        }
        out
    }
}

/// Who an equation belongs to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Owner {
    /// A user constraint.
    Constraint(ConstraintId),
    /// An arc keeping its end on its circle.
    Arc(EntityId),
    /// A soft pull towards a value (dragging, minimal perturbation).
    Soft,
}

/// One source of residual rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Equation {
    /// A constraint.
    Constraint(ConstraintId, Constraint),
    /// `|end − center| = |start − center|` for an arc.
    Arc(EntityId),
    /// `weight · (x[var] − value)`.
    Soft {
        /// Variable.
        var: usize,
        /// Target value.
        value: f64,
        /// Weight.
        weight: f64,
    },
}

impl Equation {
    /// Its owner.
    pub fn owner(&self) -> Owner {
        match *self {
            Self::Constraint(id, _) => Owner::Constraint(id),
            Self::Arc(id) => Owner::Arc(id),
            Self::Soft { .. } => Owner::Soft,
        }
    }
}

/// A residual row: value and sparse gradient over global variables.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// Residual value.
    pub value: f64,
    /// `(variable, ∂residual/∂variable)`.
    pub grad: Vec<(usize, f64)>,
}

/// The hard equations of a sketch: arcs first, then every enforced constraint.
pub fn hard_equations(sketch: &Sketch) -> Vec<Equation> {
    let arcs = sketch
        .entities
        .iter()
        .filter(|(_, r)| matches!(r.entity, Entity::Arc { .. }))
        .map(|(id, _)| Equation::Arc(*id));
    let constraints = sketch
        .constraints
        .iter()
        .filter(|(_, c)| !c.reference)
        .map(|(id, c)| Equation::Constraint(*id, c.constraint));
    arcs.chain(constraints).collect()
}

/// Gathers the local variables of one equation and evaluates it with duals.
struct Locals<'a> {
    x: &'a [f64],
    layout: &'a Layout,
    sketch: &'a Sketch,
    globals: Vec<usize>,
}

/// An equation referenced something that does not exist (or touched too many
/// variables); its rows are skipped.
struct Missing;

impl Locals<'_> {
    fn var(&mut self, global: usize) -> Result<Dual, Missing> {
        let value = *self.x.get(global).ok_or(Missing)?;
        let local = match self.globals.iter().position(|g| *g == global) {
            Some(i) => i,
            None => {
                if self.globals.len() >= MAX_LOCAL {
                    return Err(Missing);
                }
                self.globals.push(global);
                self.globals.len() - 1
            }
        };
        Ok(Dual::var(value, local))
    }

    fn point(&mut self, id: EntityId) -> Result<V2, Missing> {
        let i = *self.layout.point.get(&id).ok_or(Missing)?;
        Ok(V2 {
            x: self.var(i)?,
            y: self.var(i + 1)?,
        })
    }

    fn line(&mut self, id: EntityId) -> Result<(V2, V2), Missing> {
        match self.sketch.entity(id).map_err(|_| Missing)? {
            Entity::Line { a, b } => Ok((self.point(*a)?, self.point(*b)?)),
            _ => Err(Missing),
        }
    }

    /// Centre and radius of a circle or arc.
    fn round(&mut self, id: EntityId) -> Result<(V2, Dual), Missing> {
        match *self.sketch.entity(id).map_err(|_| Missing)? {
            Entity::Circle { center, .. } => {
                let c = self.point(center)?;
                let r = self.var(*self.layout.radius.get(&id).ok_or(Missing)?)?;
                Ok((c, r))
            }
            Entity::Arc { center, start, .. } => {
                let c = self.point(center)?;
                let s = self.point(start)?;
                Ok((c, s.minus(c).norm()))
            }
            _ => Err(Missing),
        }
    }

    fn is_line(&self, id: EntityId) -> bool {
        matches!(self.sketch.entity(id), Ok(Entity::Line { .. }))
    }
}

fn abs(d: Dual) -> Dual {
    if d.v < 0.0 { -d } else { d }
}

/// Wraps an angle difference into (−π, π] (the shift is a constant: no derivative).
fn wrap(d: Dual) -> Dual {
    let turns = (d.v / std::f64::consts::TAU).round();
    d - turns * std::f64::consts::TAU
}

fn residuals(eq: &Equation, l: &mut Locals<'_>) -> Result<Vec<Dual>, Missing> {
    use Constraint as C;
    let c = match *eq {
        Equation::Soft { var, value, weight } => {
            return Ok(vec![(l.var(var)? - value) * weight]);
        }
        Equation::Arc(id) => {
            let Entity::Arc { center, start, end } = *l.sketch.entity(id).map_err(|_| Missing)?
            else {
                return Err(Missing);
            };
            let (c, s, e) = (l.point(center)?, l.point(start)?, l.point(end)?);
            return Ok(vec![e.minus(c).norm() - s.minus(c).norm()]);
        }
        Equation::Constraint(_, c) => c,
    };
    Ok(match c {
        C::Coincident { a, b } => {
            let (p, q) = (l.point(a)?, l.point(b)?);
            vec![p.x - q.x, p.y - q.y]
        }
        C::Horizontal { line } => {
            let (a, b) = l.line(line)?;
            vec![b.y - a.y]
        }
        C::Vertical { line } => {
            let (a, b) = l.line(line)?;
            vec![b.x - a.x]
        }
        C::Parallel { a, b } | C::Perpendicular { a, b } => {
            let (a0, a1) = l.line(a)?;
            let (b0, b1) = l.line(b)?;
            let (d1, d2) = (a1.minus(a0), b1.minus(b0));
            let mix = if matches!(c, C::Parallel { .. }) {
                d1.cross(d2)
            } else {
                d1.dot(d2)
            };
            vec![mix / d2.norm()]
        }
        C::Tangent { a, b } => {
            if l.is_line(a) || l.is_line(b) {
                let (line, round) = if l.is_line(a) { (a, b) } else { (b, a) };
                let (p, q) = l.line(line)?;
                let (center, r) = l.round(round)?;
                let d = q.minus(p);
                let s = d.cross(center.minus(p)) / d.norm();
                vec![(s * s - r * r) / (abs(s) + abs(r) + 1e-12)]
            } else {
                let (c1, r1) = l.round(a)?;
                let (c2, r2) = l.round(b)?;
                let dist = c1.minus(c2).norm();
                if dist.v < r1.v.max(r2.v) {
                    vec![dist - abs(r1 - r2)]
                } else {
                    vec![dist - (r1 + r2)]
                }
            }
        }
        C::Equal { a, b } => {
            if l.is_line(a) {
                let (a0, a1) = l.line(a)?;
                let (b0, b1) = l.line(b)?;
                vec![a1.minus(a0).norm() - b1.minus(b0).norm()]
            } else {
                let (_, r1) = l.round(a)?;
                let (_, r2) = l.round(b)?;
                vec![r1 - r2]
            }
        }
        C::PointOnLine { point, line } => {
            let p = l.point(point)?;
            let (a, b) = l.line(line)?;
            let d = b.minus(a);
            vec![d.cross(p.minus(a)) / d.norm()]
        }
        C::PointOnCircle { point, circle } => {
            let p = l.point(point)?;
            let (center, r) = l.round(circle)?;
            vec![p.minus(center).norm() - r]
        }
        C::Midpoint { point, line } => {
            let p = l.point(point)?;
            let (a, b) = l.line(line)?;
            vec![p.x - (a.x + b.x) * 0.5, p.y - (a.y + b.y) * 0.5]
        }
        C::Fix { point, at } => {
            let p = l.point(point)?;
            vec![p.x - at[0], p.y - at[1]]
        }
        C::Distance { a, b, value } => {
            let (p, q) = (l.point(a)?, l.point(b)?);
            vec![q.minus(p).norm() - value]
        }
        C::HorizontalDistance { a, b, value } => {
            let (p, q) = (l.point(a)?, l.point(b)?);
            vec![q.x - p.x - value]
        }
        C::VerticalDistance { a, b, value } => {
            let (p, q) = (l.point(a)?, l.point(b)?);
            vec![q.y - p.y - value]
        }
        C::Length { line, value } => {
            let (a, b) = l.line(line)?;
            vec![b.minus(a).norm() - value]
        }
        C::Radius { circle, value } => {
            let (_, r) = l.round(circle)?;
            vec![r - value]
        }
        C::Diameter { circle, value } => {
            let (_, r) = l.round(circle)?;
            vec![r * 2.0 - value]
        }
        C::Angle { a, b, value } => {
            let (a0, a1) = l.line(a)?;
            let (b0, b1) = l.line(b)?;
            let (d1, d2) = (a1.minus(a0), b1.minus(b0));
            let theta = d1.cross(d2).atan2(d1.dot(d2));
            let scale = 0.5 * (d1.norm().v + d2.norm().v);
            vec![wrap(theta - value) * scale.max(1e-9)]
        }
    })
}

/// Evaluates `eq` at `x`; an equation that references missing entities yields no rows.
pub fn rows(eq: &Equation, sketch: &Sketch, layout: &Layout, x: &[f64]) -> Vec<Row> {
    let mut locals = Locals {
        x,
        layout,
        sketch,
        globals: Vec::new(),
    };
    let Ok(duals) = residuals(eq, &mut locals) else {
        return Vec::new();
    };
    duals
        .into_iter()
        .map(|d| Row {
            value: d.v,
            grad: locals
                .globals
                .iter()
                .zip(d.d)
                .filter(|(_, g)| *g != 0.0)
                .map(|(i, g)| (*i, g))
                .collect(),
        })
        .collect()
}

/// The global variables an equation touches (structure only, for decomposition).
pub fn vars_of(eq: &Equation, sketch: &Sketch, layout: &Layout) -> Vec<usize> {
    match *eq {
        Equation::Soft { var, .. } => vec![var],
        Equation::Arc(id) => layout.vars_of(sketch, id),
        Equation::Constraint(_, c) => {
            let mut v: Vec<usize> = c
                .entities()
                .into_iter()
                .flat_map(|e| layout.vars_of(sketch, e))
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        }
    }
}
