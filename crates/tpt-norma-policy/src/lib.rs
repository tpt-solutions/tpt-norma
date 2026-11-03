// Copyright (c) TPT Solutions
// SPDX-License-Identifier: MIT OR Apache-2.0

//! N-dimensional policy space and its consistency check (spec §4, `tpt-norma-policy`).
//!
//! A policy is a set of named dimensions, each with an inclusive integer range (an axis-aligned
//! box), plus named linear rules over those dimensions. The DSL is line based:
//!
//! ```text
//! # comments start with '#'
//! dim delta in [0, 50]
//! dim load  in [0, 1000]
//! rule per_call_cap: delta <= 50
//! rule headroom:     load + 2 * delta <= 1000
//! ```
//!
//! Operators are `<=`, `<`, `>=`, `>` and `==`. Expressions are sums of `var`, `k * var` and
//! integer constants.
//!
//! Coefficients and bounds are `i64`. A policy in fixed-point units (for example micro-units)
//! must be scaled before it is written here.
//!
//! [`PolicySpace::check_consistency`] uses `tpt-norma-linear`'s [`check_contradictions`]. Each rule
//! and each dimension's bounds form a named group, and the check reports which groups can never
//! hold together. It also reports whether the whole space is empty.

use std::collections::BTreeMap;

use tpt_norma_linear::{check_contradictions, Constraint, Linear, NamedConstraints, Relation};

/// A parse or compile error, with the 1-based line it occurred on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for PolicyError {}

/// One dimension of the policy space, with an inclusive range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dimension {
    pub name: String,
    pub min: i64,
    pub max: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Le,
    Lt,
    Ge,
    Gt,
    Eq,
}

/// A named linear rule `lhs op rhs`, where `lhs` and `rhs` are linear expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub name: String,
    pub lhs: Linear,
    pub op: Op,
    pub rhs: i64,
}

/// A policy space: dimensions and rules.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PolicySpace {
    pub dimensions: Vec<Dimension>,
    pub rules: Vec<Rule>,
}

/// The result of [`PolicySpace::check_consistency`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consistency {
    /// `true` when no group contradicts another and the whole space is non-empty.
    pub consistent: bool,
    /// `None` if the solver could not decide; the space must then be treated as unverified.
    pub decided: bool,
    /// Pairs of group labels that can never hold together. A label is a rule name, or
    /// `dim:<name>` for a dimension's bounds.
    pub contradicting_pairs: Vec<(String, String)>,
}

impl PolicySpace {
    /// Parse the DSL. Returns the first error found.
    pub fn parse(source: &str) -> Result<Self, PolicyError> {
        let mut space = PolicySpace::default();
        for (index, raw) in source.lines().enumerate() {
            let line_no = index + 1;
            let text = raw.split('#').next().unwrap_or("").trim();
            if text.is_empty() {
                continue;
            }
            let err = |message: String| PolicyError {
                line: line_no,
                message,
            };
            if let Some(rest) = text.strip_prefix("dim ") {
                space.dimensions.push(parse_dim(rest).map_err(err)?);
            } else if let Some(rest) = text.strip_prefix("rule ") {
                space.rules.push(parse_rule(rest).map_err(err)?);
            } else {
                return Err(err(format!("expected `dim` or `rule`, found `{text}`")));
            }
        }
        Ok(space)
    }

    /// Check that every rule uses declared dimensions only, and that no name is declared twice.
    pub fn validate_names(&self) -> Result<(), PolicyError> {
        let mut declared = BTreeMap::new();
        for dim in &self.dimensions {
            if declared.insert(dim.name.as_str(), ()).is_some() {
                return Err(PolicyError {
                    line: 0,
                    message: format!("dimension `{}` is declared twice", dim.name),
                });
            }
        }
        for rule in &self.rules {
            for var in rule.lhs.terms.keys() {
                if !declared.contains_key(var.as_str()) {
                    return Err(PolicyError {
                        line: 0,
                        message: format!("rule `{}` uses undeclared dimension `{var}`", rule.name),
                    });
                }
            }
        }
        Ok(())
    }

    /// Whether `point` (a value for each dimension, by name) lies inside every dimension's range
    /// and satisfies every rule. A dimension missing from `point` is treated as not admitted.
    pub fn admits(&self, point: &BTreeMap<String, i64>) -> bool {
        let in_boxes = self.dimensions.iter().all(|d| {
            point
                .get(&d.name)
                .is_some_and(|&v| v >= d.min && v <= d.max)
        });
        in_boxes
            && self.rules.iter().all(|rule| {
                let Some(value) = eval(&rule.lhs, point) else {
                    return false;
                };
                match rule.op {
                    Op::Le => value <= rule.rhs,
                    Op::Lt => value < rule.rhs,
                    Op::Ge => value >= rule.rhs,
                    Op::Gt => value > rule.rhs,
                    Op::Eq => value == rule.rhs,
                }
            })
    }

    /// Check whether the groups can hold together, using `tpt-norma-linear`'s contradiction check.
    pub fn check_consistency(&self) -> Result<Consistency, PolicyError> {
        self.validate_names()?;

        let mut groups = Vec::new();
        for dim in &self.dimensions {
            let x = Linear::var(&dim.name);
            groups.push(NamedConstraints {
                label: format!("dim:{}", dim.name),
                constraints: vec![
                    Constraint {
                        expr: x.sub(&Linear::constant_only(dim.min)),
                        relation: Relation::Ge,
                    },
                    Constraint {
                        expr: x.sub(&Linear::constant_only(dim.max)),
                        relation: Relation::Le,
                    },
                ],
            });
        }
        for rule in &self.rules {
            groups.push(NamedConstraints {
                label: rule.name.clone(),
                constraints: rule_constraints(rule),
            });
        }

        let report = check_contradictions(&groups);
        let decided = report.overall_unsat.is_some();
        // If the groups were jointly unsatisfiable, the space is empty. A pair alone is not
        // enough to say that.
        let consistent = report.overall_unsat == Some(false) && report.pairs.is_empty();
        Ok(Consistency {
            consistent,
            decided,
            contradicting_pairs: report.pairs.into_iter().map(|p| (p.a, p.b)).collect(),
        })
    }
}

/// The constraints for one rule. `Eq` becomes a single `Eq` constraint.
fn rule_constraints(rule: &Rule) -> Vec<Constraint> {
    let expr = rule.lhs.sub(&Linear::constant_only(rule.rhs));
    let relation = match rule.op {
        Op::Le => Relation::Le,
        Op::Lt => Relation::Lt,
        Op::Ge => Relation::Ge,
        Op::Gt => Relation::Gt,
        Op::Eq => Relation::Eq,
    };
    vec![Constraint { expr, relation }]
}

fn eval(expr: &Linear, point: &BTreeMap<String, i64>) -> Option<i64> {
    let mut total = 0i64;
    for (var, coeff) in &expr.terms {
        let value = *point.get(var)?;
        total = total.checked_add(value.checked_mul(*coeff)?)?;
    }
    Some(total)
}

fn parse_dim(text: &str) -> Result<Dimension, String> {
    // `<name> in [<min>, <max>]`
    let (name, range) = text
        .split_once(" in ")
        .ok_or_else(|| format!("expected `dim <name> in [min, max]`, found `dim {text}`"))?;
    let name = name.trim();
    check_ident(name)?;
    let range = range.trim();
    let inner = range
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix(']'))
        .ok_or_else(|| format!("range must be written `[min, max]`, found `{range}`"))?;
    let (min, max) = inner
        .split_once(',')
        .ok_or_else(|| format!("range must have a comma, found `{range}`"))?;
    let min = parse_int(min.trim())?;
    let max = parse_int(max.trim())?;
    if min > max {
        return Err(format!(
            "dimension `{name}` has min {min} greater than max {max}"
        ));
    }
    Ok(Dimension {
        name: name.to_string(),
        min,
        max,
    })
}

fn parse_rule(text: &str) -> Result<Rule, String> {
    // `<name>: <expr> <op> <int>`
    let (name, body) = text
        .split_once(':')
        .ok_or_else(|| format!("expected `rule <name>: <expr> <op> <int>`, found `rule {text}`"))?;
    let name = name.trim();
    check_ident(name)?;

    let (lhs_text, op, rhs_text) = split_op(body.trim())
        .ok_or_else(|| format!("rule `{name}` needs one of <=, <, >=, >, =="))?;
    let lhs = parse_expr(lhs_text.trim())?;
    let rhs = parse_int(rhs_text.trim())?;
    Ok(Rule {
        name: name.to_string(),
        lhs,
        op,
        rhs,
    })
}

/// Split `lhs op rhs` on the first relational operator. Two-character operators are tried first.
fn split_op(body: &str) -> Option<(&str, Op, &str)> {
    for (token, op) in [
        ("<=", Op::Le),
        (">=", Op::Ge),
        ("==", Op::Eq),
        ("<", Op::Lt),
        (">", Op::Gt),
    ] {
        if let Some(index) = body.find(token) {
            return Some((&body[..index], op, &body[index + token.len()..]));
        }
    }
    None
}

/// Parse a sum like `load + 2 * delta - 3`.
fn parse_expr(text: &str) -> Result<Linear, String> {
    let spaced = text
        .replace('+', " + ")
        .replace('-', " - ")
        .replace('*', " * ");
    let tokens: Vec<&str> = spaced.split_whitespace().collect();
    if tokens.is_empty() {
        return Err("empty expression".into());
    }

    let mut total = Linear::constant_only(0);
    let mut sign = 1i64;
    let mut i = 0;
    let mut expect_term = true;
    while i < tokens.len() {
        let token = tokens[i];
        match token {
            "+" | "-" => {
                if expect_term {
                    return Err(format!("unexpected `{token}` in `{text}`"));
                }
                sign = if token == "+" { 1 } else { -1 };
                expect_term = true;
                i += 1;
            }
            _ => {
                if !expect_term {
                    return Err(format!("expected `+` or `-` before `{token}` in `{text}`"));
                }
                // `k * var`, a bare `var`, or a bare integer.
                let (term, consumed) = if tokens.get(i + 1) == Some(&"*") {
                    let coeff = parse_int(token)?;
                    let var = tokens
                        .get(i + 2)
                        .ok_or_else(|| format!("missing variable after `{token} *`"))?;
                    check_ident(var)?;
                    (Linear::var(var).scale(coeff), 3)
                } else if let Ok(value) = parse_int(token) {
                    (Linear::constant_only(value), 1)
                } else {
                    check_ident(token)?;
                    (Linear::var(token), 1)
                };
                total = total.add(&term.scale(sign));
                sign = 1;
                expect_term = false;
                i += consumed;
            }
        }
    }
    if expect_term {
        return Err(format!("expression `{text}` ends with an operator"));
    }
    Ok(total)
}

fn parse_int(text: &str) -> Result<i64, String> {
    text.parse::<i64>()
        .map_err(|_| format!("expected an integer, found `{text}`"))
}

fn check_ident(text: &str) -> Result<(), String> {
    let mut chars = text.chars();
    let starts_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if starts_ok && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Ok(())
    } else {
        Err(format!("`{text}` is not a valid name"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(pairs: &[(&str, i64)]) -> BTreeMap<String, i64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    const CONSISTENT: &str = "\
dim delta in [0, 50]
dim load in [0, 1000]
rule per_call_cap: delta <= 50
rule headroom: load + 2 * delta <= 1000
";

    #[test]
    fn a_consistent_policy_parses_and_is_reported_consistent() {
        let space = PolicySpace::parse(CONSISTENT).expect("parses");
        assert_eq!(space.dimensions.len(), 2);
        assert_eq!(space.rules.len(), 2);
        let check = space.check_consistency().expect("names valid");
        assert!(check.decided);
        assert!(check.consistent, "{check:?}");
        assert!(check.contradicting_pairs.is_empty());
    }

    #[test]
    fn contradictory_rules_are_reported_by_name() {
        let source = format!("{CONSISTENT}rule big_budget: delta >= 60\n");
        let space = PolicySpace::parse(&source).expect("parses");
        let check = space.check_consistency().expect("names valid");
        assert!(!check.consistent);
        assert!(check
            .contradicting_pairs
            .iter()
            .any(|(a, b)| (a == "per_call_cap" && b == "big_budget")
                || (a == "big_budget" && b == "per_call_cap")));
    }

    #[test]
    fn a_dimension_bound_can_contradict_a_rule() {
        let source = "dim delta in [0, 10]\nrule floor: delta >= 20\n";
        let space = PolicySpace::parse(source).expect("parses");
        let check = space.check_consistency().expect("names valid");
        assert!(!check.consistent);
        assert!(check
            .contradicting_pairs
            .iter()
            .any(|(a, b)| a == "dim:delta" || b == "dim:delta"));
    }

    #[test]
    fn admits_checks_boxes_and_rules() {
        let space = PolicySpace::parse(CONSISTENT).expect("parses");
        assert!(space.admits(&point(&[("delta", 10), ("load", 100)])));
        assert!(!space.admits(&point(&[("delta", 51), ("load", 100)])));
        assert!(
            !space.admits(&point(&[("delta", 10), ("load", 999)])),
            "headroom rule"
        );
        assert!(!space.admits(&point(&[("delta", 10)])), "missing dimension");
    }

    #[test]
    fn parse_errors_name_the_line() {
        let err = PolicySpace::parse("dim x in [5, 1]\n").unwrap_err();
        assert_eq!(err.line, 1);
        let err = PolicySpace::parse("dim x in [0, 1]\nrule r: x <> 3\n").unwrap_err();
        assert_eq!(err.line, 2);
        let err = PolicySpace::parse("widget x\n").unwrap_err();
        assert_eq!(err.line, 1);
    }

    #[test]
    fn rules_over_undeclared_dimensions_are_rejected() {
        let space = PolicySpace::parse("dim x in [0, 1]\nrule r: y <= 3\n").expect("parses");
        assert!(space.check_consistency().is_err());
    }

    #[test]
    fn duplicate_dimensions_are_rejected() {
        let space = PolicySpace::parse("dim x in [0, 1]\ndim x in [0, 2]\n").expect("parses");
        assert!(space.validate_names().is_err());
    }
}
