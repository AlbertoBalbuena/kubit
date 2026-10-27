//! A set of named parameters: dependency graph, cycle detection and ordered evaluation.

use super::ast::Expr;
use super::error::ExprError;
use super::eval::Context;
use super::parser::Definition;
use super::quantity::Quantity;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// A dependency cycle among parameters, e.g. `a -> b -> a`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("parameter cycle: {}", .path.join(" -> "))]
pub struct CycleError {
    /// The cycle, first name repeated at the end.
    pub path: Vec<String>,
}

/// Failure while evaluating a whole [`ParameterSet`].
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ParamError {
    /// The parameters depend on each other in a loop.
    #[error(transparent)]
    Cycle(#[from] CycleError),
    /// One parameter's expression failed.
    #[error("parameter `{name}`: {error}")]
    Eval {
        /// The parameter.
        name: String,
        /// Why its expression failed.
        error: ExprError,
    },
}

/// Named parameters, ordered by name (deterministic iteration).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParameterSet {
    defs: BTreeMap<String, Expr>,
}

struct Layered<'a> {
    values: &'a BTreeMap<String, Quantity>,
    external: &'a dyn Context,
}

impl Context for Layered<'_> {
    fn resolve(&self, name: &str) -> Option<Quantity> {
        self.values
            .get(name)
            .copied()
            .or_else(|| self.external.resolve(name))
    }
}

impl ParameterSet {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of parameters.
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    /// `true` if there are no parameters.
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// Defines (or redefines) a parameter, returning the previous expression if any.
    pub fn insert(&mut self, name: impl Into<String>, expr: Expr) -> Option<Expr> {
        self.defs.insert(name.into(), expr)
    }

    /// Defines a parameter from a parsed [`Definition`].
    pub fn insert_definition(&mut self, def: Definition) -> Option<Expr> {
        self.insert(def.name, def.expr)
    }

    /// The expression of `name`.
    pub fn get(&self, name: &str) -> Option<&Expr> {
        self.defs.get(name)
    }

    /// Parameter names in sorted order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.defs.keys().map(String::as_str)
    }

    /// Parameters in this set that `name` reads directly (external identifiers excluded).
    pub fn dependencies(&self, name: &str) -> BTreeSet<String> {
        self.defs
            .get(name)
            .map(|e| {
                e.references()
                    .into_iter()
                    .filter(|r| self.defs.contains_key(r))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `Ok` if the dependency graph has no cycles.
    pub fn check_cycles(&self) -> Result<(), CycleError> {
        self.evaluation_order().map(|_| ())
    }

    /// A topological order (dependencies first), deterministic: ties break by name.
    /// Iterative (Kahn), so arbitrarily large sets cannot overflow the stack.
    pub fn evaluation_order(&self) -> Result<Vec<String>, CycleError> {
        let deps: BTreeMap<&str, BTreeSet<String>> = self
            .defs
            .keys()
            .map(|n| (n.as_str(), self.dependencies(n)))
            .collect();
        let mut pending: BTreeMap<&str, usize> = deps.iter().map(|(n, d)| (*n, d.len())).collect();
        let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (n, ds) in &deps {
            for d in ds {
                dependents.entry(d.as_str()).or_default().push(n);
            }
        }
        let mut ready: BTreeSet<&str> = pending
            .iter()
            .filter(|(_, c)| **c == 0)
            .map(|(n, _)| *n)
            .collect();
        let mut order = Vec::with_capacity(self.defs.len());
        while let Some(n) = ready.pop_first() {
            order.push(n.to_owned());
            for &dep in dependents.get(n).map(Vec::as_slice).unwrap_or_default() {
                if let Some(c) = pending.get_mut(dep) {
                    *c -= 1;
                    if *c == 0 {
                        ready.insert(dep);
                    }
                }
            }
        }
        if order.len() == self.defs.len() {
            return Ok(order);
        }
        let done: BTreeSet<&str> = order.iter().map(String::as_str).collect();
        Err(find_cycle(&deps, &done))
    }

    /// Evaluates every parameter in dependency order. Identifiers outside the set resolve via
    /// `external` (e.g. measures like `Body1.height`).
    pub fn evaluate(
        &self,
        external: &dyn Context,
    ) -> Result<BTreeMap<String, Quantity>, ParamError> {
        let order = self.evaluation_order()?;
        let mut values = BTreeMap::new();
        for name in order {
            let Some(expr) = self.defs.get(&name) else {
                continue;
            };
            let q = expr
                .eval(&Layered {
                    values: &values,
                    external,
                })
                .map_err(|error| ParamError::Eval {
                    name: name.clone(),
                    error,
                })?;
            values.insert(name, q);
        }
        Ok(values)
    }
}

/// Walks unresolved nodes (each has an unresolved dependency) until one repeats.
fn find_cycle(deps: &BTreeMap<&str, BTreeSet<String>>, done: &BTreeSet<&str>) -> CycleError {
    let remaining = |n: &str| deps.contains_key(n) && !done.contains(n);
    let mut path: Vec<&str> = Vec::new();
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut cur = deps.keys().copied().find(|n| remaining(n));
    while let Some(n) = cur {
        if let Some(&at) = seen.get(n) {
            let mut cycle: Vec<String> = path
                .get(at..)
                .unwrap_or_default()
                .iter()
                .map(|s| (*s).to_owned())
                .collect();
            cycle.push(n.to_owned());
            return CycleError { path: cycle };
        }
        seen.insert(n, path.len());
        path.push(n);
        cur = deps
            .get(n)
            .and_then(|ds| ds.iter().map(String::as_str).find(|d| remaining(d)));
    }
    CycleError {
        path: path.iter().map(|s| (*s).to_owned()).collect(),
    }
}
