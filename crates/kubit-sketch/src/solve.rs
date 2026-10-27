//! The solver: Levenberg-Marquardt per connected component, then a diagnosis of
//! degrees of freedom, redundant and conflicting constraints.
//!
//! Damping (`λ`) keeps steps small, so an under-constrained sketch moves as little
//! as it can to satisfy its constraints: geometry does not jump. Dragging adds soft
//! equations (the dragged point towards the pointer, every other variable towards
//! where it was) and then re-solves the hard equations alone, so the result is exact.

use crate::equations::{Equation, Layout, Owner, Row, hard_equations, rows, vars_of};
use crate::model::{ConstraintId, EntityId, Sketch, SketchError};
use indexmap::IndexMap;
use nalgebra::{DMatrix, DVector};
use serde::Serialize;

/// Solver settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolveOptions {
    /// Largest residual accepted as solved, mm.
    pub tolerance: f64,
    /// Iteration limit per component.
    pub max_iterations: usize,
    /// Above this many variables the per-constraint diagnosis is skipped (cost grows
    /// with the cube of the size); the DOF count is still exact.
    pub diagnose_limit: usize,
}

impl Default for SolveOptions {
    fn default() -> Self {
        Self {
            tolerance: 1e-9,
            max_iterations: 200,
            diagnose_limit: 400,
        }
    }
}

/// How free an entity still is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "dof", rename_all = "snake_case")]
pub enum Freedom {
    /// Nothing can move it.
    Fully,
    /// It can still move in this many independent ways.
    Under(usize),
    /// A constraint on it conflicts with the others.
    Over,
}

/// What a solve found.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SolveReport {
    /// Every hard equation is within tolerance.
    pub converged: bool,
    /// Iterations, summed over components.
    pub iterations: usize,
    /// Largest remaining residual, mm.
    pub residual: f64,
    /// Remaining degrees of freedom of the whole sketch.
    pub dof: usize,
    /// Freedom of every entity, in entity order.
    pub entities: IndexMap<EntityId, Freedom>,
    /// Constraints implied by earlier ones (removing them changes nothing).
    pub redundant: Vec<ConstraintId>,
    /// Constraints that cannot hold together with earlier ones.
    pub conflicting: Vec<ConstraintId>,
}

/// Union-find over variables.
struct Groups(Vec<usize>);

impl Groups {
    fn new(n: usize) -> Self {
        Self((0..n).collect())
    }

    fn find(&mut self, i: usize) -> usize {
        let mut root = i;
        while let Some(&p) = self.0.get(root) {
            if p == root {
                break;
            }
            root = p;
        }
        let mut j = i;
        while let Some(&p) = self.0.get(j) {
            if p == root {
                break;
            }
            if let Some(slot) = self.0.get_mut(j) {
                *slot = root;
            }
            j = p;
        }
        root
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if let Some(slot) = self.0.get_mut(ra.max(rb)) {
            *slot = ra.min(rb);
        }
    }
}

/// Splits equations into independent components: `(variables, equations)` each,
/// in a deterministic order (by smallest variable).
fn components(
    eqs: &[Equation],
    sketch: &Sketch,
    layout: &Layout,
) -> Vec<(Vec<usize>, Vec<Equation>)> {
    let mut groups = Groups::new(layout.len);
    let touched: Vec<Vec<usize>> = eqs.iter().map(|e| vars_of(e, sketch, layout)).collect();
    for vars in &touched {
        if let Some((&first, rest)) = vars.split_first() {
            for &v in rest {
                groups.union(first, v);
            }
        }
    }
    let mut by_root: std::collections::BTreeMap<usize, (Vec<usize>, Vec<Equation>)> =
        Default::default();
    for (eq, vars) in eqs.iter().zip(&touched) {
        if let Some(&first) = vars.first() {
            let root = groups.find(first);
            by_root.entry(root).or_default().1.push(*eq);
        }
    }
    for v in 0..layout.len {
        let root = groups.find(v);
        if let Some(entry) = by_root.get_mut(&root) {
            entry.0.push(v);
        }
    }
    by_root.into_values().collect()
}

fn stack(eqs: &[Equation], sketch: &Sketch, layout: &Layout, x: &[f64]) -> Vec<Row> {
    eqs.iter()
        .flat_map(|e| rows(e, sketch, layout, x))
        .collect()
}

fn max_abs(rows: &[Row]) -> f64 {
    rows.iter().map(|r| r.value.abs()).fold(0.0, f64::max)
}

/// Levenberg-Marquardt on one component; returns `(iterations, converged)`.
fn lm(
    vars: &[usize],
    eqs: &[Equation],
    sketch: &Sketch,
    layout: &Layout,
    x: &mut [f64],
    opts: &SolveOptions,
) -> (usize, bool) {
    let col: std::collections::BTreeMap<usize, usize> =
        vars.iter().enumerate().map(|(c, v)| (*v, c)).collect();
    let n = vars.len();
    let mut lambda = 1e-3;
    let mut current = stack(eqs, sketch, layout, x);
    let cost = |rs: &[Row]| rs.iter().map(|r| r.value * r.value).sum::<f64>();
    for it in 0..opts.max_iterations {
        if max_abs(&current) <= opts.tolerance {
            return (it, true);
        }
        let m = current.len();
        let mut j = DMatrix::<f64>::zeros(m, n);
        let mut r = DVector::<f64>::zeros(m);
        for (i, row) in current.iter().enumerate() {
            if let Some(slot) = r.get_mut(i) {
                *slot = row.value;
            }
            for (v, g) in &row.grad {
                if let Some(&c) = col.get(v)
                    && let Some(slot) = j.get_mut((i, c))
                {
                    *slot += g;
                }
            }
        }
        let jt = j.transpose();
        let a = &jt * &j;
        let g = &jt * &r;
        let mut improved = false;
        while lambda < 1e12 {
            let mut damped = a.clone();
            for k in 0..n {
                let diag = a.get((k, k)).copied().unwrap_or(0.0);
                if let Some(slot) = damped.get_mut((k, k)) {
                    *slot += lambda * (diag + 1e-9);
                }
            }
            let Some(step) = damped.cholesky().map(|ch| ch.solve(&(-&g))) else {
                lambda *= 4.0;
                continue;
            };
            let mut trial = x.to_vec();
            for (k, v) in vars.iter().enumerate() {
                if let (Some(slot), Some(d)) = (trial.get_mut(*v), step.get(k)) {
                    *slot += d;
                }
            }
            if trial.iter().any(|t| !t.is_finite()) {
                lambda *= 4.0;
                continue;
            }
            let next = stack(eqs, sketch, layout, &trial);
            if cost(&next) < cost(&current) {
                x.copy_from_slice(&trial);
                current = next;
                lambda = (lambda * 0.3).max(1e-12);
                improved = true;
                let small =
                    step.norm() <= 1e-15 * (1.0 + x.iter().map(|v| v.abs()).fold(0.0, f64::max));
                if small {
                    return (it + 1, max_abs(&current) <= opts.tolerance);
                }
                break;
            }
            lambda *= 4.0;
        }
        if !improved {
            return (it + 1, max_abs(&current) <= opts.tolerance);
        }
    }
    (opts.max_iterations, max_abs(&current) <= opts.tolerance)
}

/// Solves `eqs` component by component; returns `(iterations, converged)`.
fn run(
    eqs: &[Equation],
    sketch: &Sketch,
    layout: &Layout,
    x: &mut [f64],
    opts: &SolveOptions,
) -> (usize, bool) {
    let mut iterations = 0;
    let mut converged = true;
    for (vars, part) in components(eqs, sketch, layout) {
        let (it, ok) = lm(&vars, &part, sketch, layout, x, opts);
        iterations += it;
        converged &= ok;
    }
    (iterations, converged)
}

fn check_finite(sketch: &Sketch) -> Result<(), SketchError> {
    let (_, x) = Layout::of(sketch);
    if x.iter()
        .all(|v| v.is_finite() && v.abs() <= crate::model::MAX_COORD)
    {
        Ok(())
    } else {
        Err(SketchError::BadNumber("coordinates"))
    }
}

/// Solves the sketch in place and diagnoses it. A failed solve leaves the best
/// state found (never NaN) and says so in the report.
pub fn solve(sketch: &mut Sketch, opts: &SolveOptions) -> Result<SolveReport, SketchError> {
    check_finite(sketch)?;
    let (layout, mut x) = Layout::of(sketch);
    let eqs = hard_equations(sketch);
    let (iterations, _) = run(&eqs, sketch, &layout, &mut x, opts);
    layout.store(sketch, &x);
    Ok(diagnose(sketch, &layout, &x, &eqs, iterations, opts))
}

/// Drags point `point` towards `target` with minimal perturbation of the rest, then
/// solves exactly.
pub fn drag(
    sketch: &mut Sketch,
    point: EntityId,
    target: [f64; 2],
    opts: &SolveOptions,
) -> Result<SolveReport, SketchError> {
    sketch.point(point)?;
    if !(target
        .iter()
        .all(|t| t.is_finite() && t.abs() <= crate::model::MAX_COORD))
    {
        return Err(SketchError::BadNumber("drag target"));
    }
    check_finite(sketch)?;
    let (layout, mut x) = Layout::of(sketch);
    let hard = hard_equations(sketch);
    let dragged = layout.point.get(&point).copied();
    let mut soft = hard.clone();
    for (v, value) in x.iter().enumerate() {
        let (value, weight) = match dragged {
            Some(i) if v == i => (target[0], 1.0),
            Some(i) if v == i + 1 => (target[1], 1.0),
            _ => (*value, 1e-2),
        };
        soft.push(Equation::Soft {
            var: v,
            value,
            weight,
        });
    }
    let (first, _) = run(&soft, sketch, &layout, &mut x, opts);
    let (second, _) = run(&hard, sketch, &layout, &mut x, opts);
    layout.store(sketch, &x);
    Ok(diagnose(sketch, &layout, &x, &hard, first + second, opts))
}

/// Numerical rank of `m` (rows × vars) from the eigenvalues of `mᵀm`.
fn rank_of(m: &DMatrix<f64>) -> usize {
    if m.nrows() == 0 || m.ncols() == 0 {
        return 0;
    }
    let svd = m.clone().svd(false, false);
    let max = svd.singular_values.iter().fold(0.0f64, |a, b| a.max(*b));
    let tol = 1e-8 * max.max(1.0);
    svd.singular_values.iter().filter(|s| **s > tol).count()
}

fn dense(rows: &[Row], n: usize) -> DMatrix<f64> {
    let mut m = DMatrix::<f64>::zeros(rows.len(), n);
    for (i, row) in rows.iter().enumerate() {
        for (v, g) in &row.grad {
            if let Some(slot) = m.get_mut((i, *v)) {
                *slot += g;
            }
        }
    }
    m
}

fn diagnose(
    sketch: &Sketch,
    layout: &Layout,
    x: &[f64],
    eqs: &[Equation],
    iterations: usize,
    opts: &SolveOptions,
) -> SolveReport {
    let n = layout.len;
    let per_eq: Vec<(Owner, Vec<Row>)> = eqs
        .iter()
        .map(|e| (e.owner(), rows(e, sketch, layout, x)))
        .collect();
    let all: Vec<Row> = per_eq.iter().flat_map(|(_, r)| r.clone()).collect();
    let residual = max_abs(&all);
    let converged = residual <= opts.tolerance * 10.0;

    let mut redundant = Vec::new();
    let mut conflicting = Vec::new();
    let full = dense(&all, n);
    let rank = rank_of(&full);
    if n <= opts.diagnose_limit {
        // Add equations in order; one that does not raise the rank by its row count
        // is implied (redundant) or contradicts (conflicting) the earlier ones.
        let mut so_far: Vec<Row> = Vec::new();
        let mut r_prev = 0;
        for (owner, rs) in &per_eq {
            if rs.is_empty() {
                continue;
            }
            so_far.extend(rs.iter().cloned());
            let r_now = rank_of(&dense(&so_far, n));
            let independent = r_now == r_prev + rs.len();
            if !independent && let Owner::Constraint(id) = owner {
                if max_abs(rs) <= opts.tolerance * 10.0 {
                    redundant.push(*id);
                } else {
                    conflicting.push(*id);
                }
            }
            r_prev = r_now;
        }
    }
    // If the rank test found no culprit (diagnosis skipped, or the solver stalled
    // short of a solution), every unsatisfied constraint is reported instead.
    if !converged && conflicting.is_empty() {
        for (owner, rs) in &per_eq {
            if let Owner::Constraint(id) = owner
                && max_abs(rs) > opts.tolerance * 10.0
                && !conflicting.contains(id)
            {
                conflicting.push(*id);
            }
        }
    }

    // Null space of the Jacobian: the directions the sketch can still move in.
    let null = if n == 0 {
        DMatrix::<f64>::zeros(0, 0)
    } else {
        let gram = full.transpose() * &full;
        let eig = gram.symmetric_eigen();
        let max = eig.eigenvalues.iter().fold(0.0f64, |a, b| a.max(b.abs()));
        let tol = 1e-10 * max.max(1.0);
        let cols: Vec<usize> = (0..n)
            .filter(|k| eig.eigenvalues.get(*k).is_some_and(|e| e.abs() <= tol))
            .collect();
        let mut basis = DMatrix::<f64>::zeros(n, cols.len());
        for (j, k) in cols.iter().enumerate() {
            basis.set_column(j, &eig.eigenvectors.column(*k));
        }
        basis
    };
    let over: std::collections::BTreeSet<EntityId> = conflicting
        .iter()
        .filter_map(|id| sketch.constraints.get(id))
        .flat_map(|c| c.constraint.entities())
        .flat_map(|e| {
            let mut v = vec![e];
            if let Ok(entity) = sketch.entity(e) {
                v.extend(entity.points());
            }
            v
        })
        .collect();
    let entities = sketch
        .entities
        .keys()
        .map(|id| {
            let freedom = if over.contains(id) {
                Freedom::Over
            } else {
                let vars = layout.vars_of(sketch, *id);
                let mut sub = DMatrix::<f64>::zeros(vars.len(), null.ncols());
                for (i, v) in vars.iter().enumerate() {
                    if *v < null.nrows() {
                        sub.set_row(i, &null.row(*v));
                    }
                }
                match rank_of(&sub) {
                    0 => Freedom::Fully,
                    k => Freedom::Under(k),
                }
            };
            (*id, freedom)
        })
        .collect();
    SolveReport {
        converged,
        iterations,
        residual,
        dof: n.saturating_sub(rank),
        entities,
        redundant,
        conflicting,
    }
}
