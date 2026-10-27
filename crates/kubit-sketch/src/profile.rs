//! Closed regions of a sketch: the profiles that can be extruded.
//!
//! Lines and arcs that share points (or whose ends are coincident) form a graph.
//! Every connected part in which each point joins exactly two curves is a simple
//! closed loop; each circle is a loop on its own. Loops nest by containment: a loop
//! at even depth is a region's outer boundary, and the loops directly inside it are
//! its holes. Parts that branch or stay open are not profiles (v1). Construction
//! geometry is ignored.

use crate::model::{Constraint, Entity, EntityId, Sketch};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;

/// Id of a region: the smallest entity id of its outer loop (stable while that loop
/// keeps its entities).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RegionId(pub u32);

/// One step of a loop.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Seg {
    /// Straight to a point.
    LineTo([f64; 2]),
    /// A circular arc through `via` to `to`.
    ArcTo {
        /// End point.
        to: [f64; 2],
        /// A point on the arc between its ends.
        via: [f64; 2],
    },
}

/// The shape of a closed loop.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoopShape {
    /// A full circle.
    Circle {
        /// Centre.
        center: [f64; 2],
        /// Radius, mm.
        radius: f64,
    },
    /// A chain of segments from `start` back to `start`.
    Segments {
        /// First point.
        start: [f64; 2],
        /// The steps, the last one ending at `start`.
        segments: Vec<Seg>,
    },
}

/// A closed loop and the entities it is made of.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Loop {
    /// Curves of the loop, in walking order.
    pub entities: Vec<EntityId>,
    /// Its geometry.
    pub shape: LoopShape,
}

/// An extrudable region.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    /// Stable id.
    pub id: RegionId,
    /// Outer boundary.
    pub outer: Loop,
    /// Holes directly inside it.
    pub holes: Vec<Loop>,
    /// Area, mm².
    pub area: f64,
}

impl Loop {
    /// The loop as a polygon (arcs and circles sampled), for drawing and hit tests.
    pub fn polygon(&self) -> Vec<[f64; 2]> {
        polygon(&self.shape)
    }
}

impl Region {
    /// Whether `p` lies inside the outer loop and outside every hole.
    pub fn contains(&self, p: [f64; 2]) -> bool {
        contains(&self.outer.polygon(), p) && !self.holes.iter().any(|h| contains(&h.polygon(), p))
    }
}

/// Union-find over point ids, merged by coincident constraints.
fn merged_points(sketch: &Sketch) -> BTreeMap<EntityId, EntityId> {
    let mut parent: BTreeMap<EntityId, EntityId> = sketch
        .entities
        .iter()
        .filter(|(_, r)| matches!(r.entity, Entity::Point { .. }))
        .map(|(id, _)| (*id, *id))
        .collect();
    fn root(parent: &BTreeMap<EntityId, EntityId>, mut id: EntityId) -> EntityId {
        for _ in 0..parent.len() + 1 {
            match parent.get(&id) {
                Some(p) if *p != id => id = *p,
                _ => break,
            }
        }
        id
    }
    for c in sketch.constraints.values() {
        if let Constraint::Coincident { a, b } = c.constraint {
            let (ra, rb) = (root(&parent, a), root(&parent, b));
            parent.insert(ra.max(rb), ra.min(rb));
        }
    }
    let keys: Vec<EntityId> = parent.keys().copied().collect();
    keys.into_iter().map(|k| (k, root(&parent, k))).collect()
}

/// A curve of the loop graph: endpoints (merged) and the curve.
#[derive(Clone, Copy, Debug)]
struct Curve {
    id: EntityId,
    ends: [EntityId; 2],
}

fn arc_mid(sketch: &Sketch, center: EntityId, start: EntityId, end: EntityId) -> Option<[f64; 2]> {
    let (c, s, e) = (
        sketch.point(center).ok()?,
        sketch.point(start).ok()?,
        sketch.point(end).ok()?,
    );
    let r = ((s[0] - c[0]).powi(2) + (s[1] - c[1]).powi(2)).sqrt();
    let a0 = (s[1] - c[1]).atan2(s[0] - c[0]);
    let a1 = (e[1] - c[1]).atan2(e[0] - c[0]);
    let mut sweep = (a1 - a0).rem_euclid(TAU);
    if sweep <= 0.0 {
        sweep = TAU;
    }
    let mid = a0 + sweep / 2.0;
    Some([c[0] + r * mid.cos(), c[1] + r * mid.sin()])
}

/// Samples a loop into a polygon (for area and containment).
fn polygon(shape: &LoopShape) -> Vec<[f64; 2]> {
    match shape {
        LoopShape::Circle { center, radius } => (0..64)
            .map(|i| {
                let t = TAU * f64::from(i) / 64.0;
                [center[0] + radius * t.cos(), center[1] + radius * t.sin()]
            })
            .collect(),
        LoopShape::Segments { start, segments } => {
            let mut out = vec![*start];
            let mut from = *start;
            for seg in segments {
                match *seg {
                    Seg::LineTo(to) => {
                        out.push(to);
                        from = to;
                    }
                    Seg::ArcTo { to, via } => {
                        out.extend(sample_arc(from, via, to));
                        from = to;
                    }
                }
            }
            out.pop();
            out
        }
    }
}

/// Circle through `a`, `m`, `b`: centre, radius, start angle and signed sweep from
/// `a` through `m` to `b` (positive counter-clockwise). `None` when collinear.
fn arc_geometry(a: [f64; 2], m: [f64; 2], b: [f64; 2]) -> Option<([f64; 2], f64, f64, f64)> {
    let d = 2.0 * (a[0] * (m[1] - b[1]) + m[0] * (b[1] - a[1]) + b[0] * (a[1] - m[1]));
    if d.abs() < 1e-12 {
        return None;
    }
    let sq = |p: [f64; 2]| p[0] * p[0] + p[1] * p[1];
    let ux = (sq(a) * (m[1] - b[1]) + sq(m) * (b[1] - a[1]) + sq(b) * (a[1] - m[1])) / d;
    let uy = (sq(a) * (b[0] - m[0]) + sq(m) * (a[0] - b[0]) + sq(b) * (m[0] - a[0])) / d;
    let r = ((a[0] - ux).powi(2) + (a[1] - uy).powi(2)).sqrt();
    let ang = |p: [f64; 2]| (p[1] - uy).atan2(p[0] - ux);
    let (t0, tm, t1) = (ang(a), ang(m), ang(b));
    let ccw = (tm - t0).rem_euclid(TAU) < (t1 - t0).rem_euclid(TAU);
    let sweep = if ccw {
        (t1 - t0).rem_euclid(TAU)
    } else {
        -(t0 - t1).rem_euclid(TAU)
    };
    Some(([ux, uy], r, t0, sweep))
}

/// Points of the arc from `a` through `m` to `b`, excluding `a`, including `b`.
fn sample_arc(a: [f64; 2], m: [f64; 2], b: [f64; 2]) -> Vec<[f64; 2]> {
    let Some(([ux, uy], r, t0, sweep)) = arc_geometry(a, m, b) else {
        return vec![m, b];
    };
    (1..=16)
        .map(|i| {
            let t = t0 + sweep * f64::from(i) / 16.0;
            [ux + r * t.cos(), uy + r * t.sin()]
        })
        .collect()
}

/// Exact area of a loop: the polygon of its vertices plus, for each arc, the
/// circular segment between the arc and its chord.
fn loop_area(shape: &LoopShape) -> f64 {
    match shape {
        LoopShape::Circle { radius, .. } => std::f64::consts::PI * radius * radius,
        LoopShape::Segments { start, segments } => {
            let mut vertices = vec![*start];
            let mut bulges = 0.0;
            let mut from = *start;
            for seg in segments {
                let to = match *seg {
                    Seg::LineTo(to) => to,
                    Seg::ArcTo { to, via } => {
                        if let Some((_, r, _, sweep)) = arc_geometry(from, via, to) {
                            bulges += 0.5 * r * r * (sweep - sweep.sin());
                        }
                        to
                    }
                };
                vertices.push(to);
                from = to;
            }
            vertices.pop();
            (signed_area(&vertices) + bulges).abs()
        }
    }
}

fn signed_area(poly: &[[f64; 2]]) -> f64 {
    poly.iter()
        .zip(poly.iter().cycle().skip(1))
        .map(|(p, q)| p[0] * q[1] - q[0] * p[1])
        .sum::<f64>()
        * 0.5
}

fn contains(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut inside = false;
    let previous = poly.iter().cycle().skip(poly.len().saturating_sub(1));
    for (a, b) in poly.iter().zip(previous) {
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

/// Every closed region of the sketch, sorted by id.
pub fn profiles(sketch: &Sketch) -> Vec<Region> {
    let merged = merged_points(sketch);
    let node = |p: EntityId| merged.get(&p).copied().unwrap_or(p);
    let mut curves = Vec::new();
    let mut loops: Vec<Loop> = Vec::new();
    for (id, r) in &sketch.entities {
        if r.construction {
            continue;
        }
        match r.entity {
            Entity::Line { a, b } => curves.push(Curve {
                id: *id,
                ends: [node(a), node(b)],
            }),
            Entity::Arc { start, end, .. } => curves.push(Curve {
                id: *id,
                ends: [node(start), node(end)],
            }),
            Entity::Circle { center, radius } => {
                if let Ok(c) = sketch.point(center) {
                    loops.push(Loop {
                        entities: vec![*id],
                        shape: LoopShape::Circle { center: c, radius },
                    });
                }
            }
            Entity::Point { .. } => {}
        }
    }
    // Incidence: node → curves.
    let mut at: BTreeMap<EntityId, Vec<usize>> = BTreeMap::new();
    for (i, c) in curves.iter().enumerate() {
        for e in c.ends {
            at.entry(e).or_default().push(i);
        }
    }
    let mut used = BTreeSet::new();
    for first in 0..curves.len() {
        if used.contains(&first) {
            continue;
        }
        // Walk; a node of degree ≠ 2 or a self-loop curve breaks the loop.
        let Some(c0) = curves.get(first).copied() else {
            continue;
        };
        let mut order = vec![first];
        let mut from = c0.ends[0];
        let mut to = c0.ends[1];
        let start = from;
        let mut closed = false;
        let mut ok = from != to;
        while ok && order.len() <= curves.len() {
            if to == start {
                closed = true;
                break;
            }
            let next: Vec<usize> = at.get(&to).cloned().unwrap_or_default();
            if next.len() != 2 {
                ok = false;
                break;
            }
            let Some(&n) = next.iter().find(|i| Some(**i) != order.last().copied()) else {
                ok = false;
                break;
            };
            if order.contains(&n) {
                ok = false;
                break;
            }
            order.push(n);
            let Some(c) = curves.get(n) else { break };
            from = to;
            to = if c.ends[0] == from {
                c.ends[1]
            } else {
                c.ends[0]
            };
        }
        for i in &order {
            used.insert(*i);
        }
        if !(ok && closed) || at.get(&start).is_none_or(|v| v.len() != 2) {
            continue;
        }
        // Geometry, walking from `start`.
        let Ok(start_pos) = sketch.point(start) else {
            continue;
        };
        let mut segments = Vec::new();
        let mut here = start;
        let mut entities = Vec::new();
        let mut good = true;
        for i in &order {
            let Some(c) = curves.get(*i) else {
                good = false;
                break;
            };
            let there = if c.ends[0] == here {
                c.ends[1]
            } else {
                c.ends[0]
            };
            let Ok(to_pos) = sketch.point(there) else {
                good = false;
                break;
            };
            let seg = match sketch.entity(c.id) {
                Ok(Entity::Line { .. }) => Seg::LineTo(to_pos),
                Ok(Entity::Arc {
                    center,
                    start: s,
                    end: e,
                }) => match arc_mid(sketch, *center, *s, *e) {
                    Some(via) => Seg::ArcTo { to: to_pos, via },
                    None => {
                        good = false;
                        break;
                    }
                },
                _ => {
                    good = false;
                    break;
                }
            };
            segments.push(seg);
            entities.push(c.id);
            here = there;
        }
        if good {
            loops.push(Loop {
                entities,
                shape: LoopShape::Segments {
                    start: start_pos,
                    segments,
                },
            });
        }
    }

    let polys: Vec<Vec<[f64; 2]>> = loops.iter().map(|l| polygon(&l.shape)).collect();
    let areas: Vec<f64> = loops.iter().map(|l| loop_area(&l.shape)).collect();
    let inside = |i: usize, j: usize| -> bool {
        // Is loop i inside loop j?
        i != j
            && areas.get(i) < areas.get(j)
            && polys
                .get(i)
                .and_then(|p| p.first())
                .is_some_and(|p| polys.get(j).is_some_and(|q| contains(q, *p)))
    };
    let depth: Vec<usize> = (0..loops.len())
        .map(|i| (0..loops.len()).filter(|j| inside(i, *j)).count())
        .collect();
    let mut regions = Vec::new();
    for (i, l) in loops.iter().enumerate() {
        let d = depth.get(i).copied().unwrap_or(0);
        if d % 2 != 0 {
            continue;
        }
        let holes: Vec<usize> = (0..loops.len())
            .filter(|h| depth.get(*h).copied() == Some(d + 1) && inside(*h, i))
            .collect();
        let id = l
            .entities
            .iter()
            .min()
            .map_or(RegionId(u32::MAX), |e| RegionId(e.0));
        let area = areas.get(i).copied().unwrap_or(0.0)
            - holes.iter().filter_map(|h| areas.get(*h)).sum::<f64>();
        regions.push(Region {
            id,
            outer: l.clone(),
            holes: holes
                .iter()
                .filter_map(|h| loops.get(*h).cloned())
                .collect(),
            area,
        });
    }
    regions.sort_by_key(|r| r.id);
    regions
}
