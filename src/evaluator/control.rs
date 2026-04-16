// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::scope::Scope;
use crate::value::*;

use super::{Evaluator, values_equal, can_adopt_cross_category};

impl Evaluator {
    /// Evaluate a case/when expression (§5.10, §5.9).
    ///
    /// Three modes:
    /// - `CaseMode::Value` — value matching (`case expr`)
    /// - `CaseMode::Type` — type dispatch on untagged unions (`case type expr`)
    /// - `CaseMode::Named` — variant dispatch on tagged unions (`case named expr`)
    pub(crate) fn eval_case(
        &mut self,
        mode: &CaseMode,
        scrutinee: &Node,
        when_clauses: &[WhenClause],
        else_branch: &Node,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        let scrut_val = self.eval_node(scrutinee, scope, exclude)?;

        if scrut_val.is_undefined() {
            return Err(UzonError::runtime("cannot match against undefined", node.span.line, node.span.col));
        }

        match mode {
            CaseMode::Value => {
                // §11.2.1: cannot branch on untagged union values with plain case
                if matches!(&scrut_val, Value::Union(_)) {
                    return Err(UzonError::type_error(
                        "cannot branch on untagged union with 'case'; use 'case type' instead",
                        node.span.line, node.span.col,
                    ));
                }
                self.eval_case_value(when_clauses, &scrut_val, else_branch, scope, exclude, node)
            }
            CaseMode::Named => {
                self.eval_case_named(scrutinee, when_clauses, &scrut_val, else_branch, scope, exclude, node)
            }
            CaseMode::Type => {
                self.eval_case_type(scrutinee, when_clauses, &scrut_val, else_branch, scope, exclude, node)
            }
        }
    }

    /// `case expr` — value matching.
    fn eval_case_value(
        &mut self,
        when_clauses: &[WhenClause],
        scrut_val: &Value,
        else_branch: &Node,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        let mut matched_result: Option<Value> = None;
        for wc in when_clauses {
            self.eval_case_when_value(wc, scrut_val, &mut matched_result, scope, exclude)?;
        }
        self.typecheck_case_branches(matched_result, when_clauses, else_branch, scope, exclude, node)
    }

    /// `case named expr` — variant dispatch on tagged unions with branch narrowing (§5.10).
    ///
    /// Branch narrowing: inside a `when` branch, the scrutinee is narrowed to
    /// the matched variant's inner value. Non-selected branches are speculatively
    /// evaluated (§5.9/§D.5) with narrowed scope for type checking — RuntimeError
    /// suppressed, TypeError always propagated.
    fn eval_case_named(
        &mut self,
        scrutinee: &Node,
        when_clauses: &[WhenClause],
        scrut_val: &Value,
        else_branch: &Node,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        let tu = match scrut_val {
            Value::TaggedUnion(tu) => tu,
            _ => {
                return Err(UzonError::type_error(
                    format!("'case named' requires tagged union scrutinee, got {}", scrut_val.type_name()),
                    node.span.line, node.span.col,
                ));
            }
        };

        // Extract scrutinee name for narrowing.
        let scrut_name = if let NodeKind::Identifier { name } = &scrutinee.kind {
            Some(name.as_str())
        } else {
            None
        };
        let narrowed_val = Self::unwrap_union_owned(scrut_val.clone());

        // Collect variant info for speculative eval narrowing.
        let variants = tu.variants.clone();

        let mut matched_result: Option<Value> = None;

        for wc in when_clauses {
            let variant_name = match &wc.value.kind {
                NodeKind::Identifier { name } => name.as_str(),
                _ => return Err(UzonError::syntax(
                    "expected variant name after 'when' in 'case named'",
                    wc.value.span.line, wc.value.span.col,
                )),
            };

            // Validate variant exists — all when clauses checked, not just the match.
            if !variants.contains_key(variant_name) {
                return Err(UzonError::type_error(
                    format!("'{}' is not a valid variant of this tagged union", variant_name),
                    wc.value.span.line, wc.value.span.col,
                ));
            }

            if matched_result.is_none() && tu.tag == variant_name {
                // §5.10: evaluate the matched branch with the scrutinee narrowed
                // to the matched variant's inner value.
                matched_result = Some(if let Some(name) = scrut_name {
                    let mut narrowed_scope = Scope::with_parent(scope.clone());
                    narrowed_scope.define(name, narrowed_val.clone());
                    self.eval_node(&wc.result, &mut narrowed_scope, exclude)?
                } else {
                    self.eval_node(&wc.result, scope, exclude)?
                });
            }
        }

        if let Some(matched) = matched_result {
            // §5.9/§D.5: speculatively evaluate non-selected branches with narrowed scope.
            // Each branch narrows to the variant's inner type.
            for wc in when_clauses {
                let variant_name = match &wc.value.kind {
                    NodeKind::Identifier { name } => name.as_str(),
                    _ => continue,
                };
                if variant_name == tu.tag { continue; } // skip matched branch
                let branch_result = if let Some(name) = scrut_name {
                    let narrowed = self.create_narrowed_value_for_variant(variant_name, &variants, &narrowed_val);
                    let mut narrowed_scope = Scope::with_parent(scope.clone());
                    narrowed_scope.define(name, narrowed);
                    self.eval_node(&wc.result, &mut narrowed_scope, exclude)
                } else {
                    self.eval_node(&wc.result, scope, exclude)
                };
                match branch_result {
                    Ok(branch_val) => Self::check_branch_types(&matched, &branch_val, node)?,
                    Err(e) if e.is_runtime() => {}
                    Err(e) => return Err(e),
                }
            }
            // Speculative eval of else branch
            {
                let else_result = if let Some(name) = scrut_name {
                    let mut narrowed_scope = Scope::with_parent(scope.clone());
                    narrowed_scope.define(name, narrowed_val.clone());
                    self.eval_node(else_branch, &mut narrowed_scope, exclude)
                } else {
                    self.eval_node(else_branch, scope, exclude)
                };
                match else_result {
                    Ok(else_val) => Self::check_branch_types(&matched, &else_val, node)?,
                    Err(e) if e.is_runtime() => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(matched)
        } else {
            // §5.10: else branch — narrow scrutinee to inner value.
            let else_val = if let Some(name) = scrut_name {
                let mut narrowed_scope = Scope::with_parent(scope.clone());
                narrowed_scope.define(name, narrowed_val.clone());
                self.eval_node(else_branch, &mut narrowed_scope, exclude)?
            } else {
                self.eval_node(else_branch, scope, exclude)?
            };
            // Speculative eval of when branches
            for wc in when_clauses {
                let variant_name = match &wc.value.kind {
                    NodeKind::Identifier { name } => name.as_str(),
                    _ => continue,
                };
                let branch_result = if let Some(name) = scrut_name {
                    let narrowed = self.create_narrowed_value_for_variant(variant_name, &variants, &narrowed_val);
                    let mut narrowed_scope = Scope::with_parent(scope.clone());
                    narrowed_scope.define(name, narrowed);
                    self.eval_node(&wc.result, &mut narrowed_scope, exclude)
                } else {
                    self.eval_node(&wc.result, scope, exclude)
                };
                match branch_result {
                    Ok(branch_val) => Self::check_branch_types(&else_val, &branch_val, node)?,
                    Err(e) if e.is_runtime() => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(else_val)
        }
    }

    /// `case type expr` — type dispatch with branch narrowing (§5.10).
    ///
    /// Works on any value, consistent with `is type` (§5.2).
    /// - Untagged unions: dispatches on inner value's type; validates `when`
    ///   types against member types.
    /// - Tagged unions: transparent (§3.7.1) — dispatches on inner value's type;
    ///   validates `when` types against the set of distinct inner types across
    ///   all variants.
    /// - Other values: dispatches on runtime type with no validation.
    ///
    /// Branch narrowing (§5.10): inside the matched `when` branch, the scrutinee
    /// is narrowed to its inner value (unwrapped from union/tagged union). This
    /// resolves the §3.7.1/§3.7.2 tension — binding (`is`) preserves the wrapper
    /// in general, but inside a `case type` branch the scrutinee is already
    /// narrowed, so `x is scrutinee` yields the inner value directly.
    /// §D.5: Non-selected branches are speculatively evaluated with narrowed
    /// scope — RuntimeError is suppressed, TypeError is always propagated.
    /// Cross-branch type checking is performed via check_branch_types to ensure
    /// all branches return the same type (§5.10).
    fn eval_case_type(
        &mut self,
        scrutinee: &Node,
        when_clauses: &[WhenClause],
        scrut_val: &Value,
        else_branch: &Node,
        scope: &mut Scope,
        exclude: Option<&str>,
        _node: &Node,
    ) -> Result<Value> {
        // Determine the runtime type to match against.
        // §3.7.1: tagged unions are transparent for case type (not in exceptions list).
        // For unions/tagged unions, collect valid member types for when-clause validation.
        let (actual_type, valid_types): (String, Option<Vec<String>>) = match scrut_val {
            Value::Union(u) => (Self::specific_type_name(&u.value), Some(u.types.clone())),
            Value::TaggedUnion(tu) => {
                // §5.10: for tagged unions, valid types are the set of distinct inner types
                // across all variants.
                let inner_types: Vec<String> = {
                    let mut seen = std::collections::HashSet::new();
                    tu.variants.values()
                        .filter_map(|v| v.as_ref())
                        .filter(|t| seen.insert(t.to_string()))
                        .cloned()
                        .collect()
                };
                (Self::specific_type_name(&tu.value), Some(inner_types))
            }
            other => (Self::specific_type_name(other), None),
        };

        // For narrowing: extract scrutinee name and unwrapped inner value.
        let scrut_name = if let NodeKind::Identifier { name } = &scrutinee.kind {
            Some(name.as_str())
        } else {
            None
        };
        let narrowed_val = Self::unwrap_union_owned(scrut_val.clone());

        let mut matched_result: Option<Value> = None;

        for wc in when_clauses {
            let type_name = match &wc.value.kind {
                NodeKind::Identifier { name } => name.as_str(),
                _ => return Err(UzonError::syntax(
                    "expected type name after 'when' in 'case type'",
                    wc.value.span.line, wc.value.span.col,
                )),
            };
            // For unions/tagged unions, validate type name is a member type
            if let Some(ref types) = valid_types {
                if !types.iter().any(|t| t == type_name) {
                    return Err(UzonError::type_error(
                        format!("'{}' is not a member type of this union", type_name),
                        wc.value.span.line, wc.value.span.col,
                    ));
                }
            }
            if matched_result.is_none() && actual_type == type_name {
                // §5.10: evaluate the matched branch with the scrutinee narrowed
                // to its inner value (unwrapped from union/tagged union).
                matched_result = Some(if let Some(name) = scrut_name {
                    let mut narrowed_scope = Scope::with_parent(scope.clone());
                    narrowed_scope.define(name, narrowed_val.clone());
                    self.eval_node(&wc.result, &mut narrowed_scope, exclude)?
                } else {
                    self.eval_node(&wc.result, scope, exclude)?
                });
            }
        }

        // §5.9/§5.10/§D.5: all branches must produce the same result type.
        // Non-matched branches are speculatively evaluated with narrowed scope;
        // RuntimeError is suppressed, TypeError is always propagated.
        if let Some(matched) = matched_result {
            // Speculative eval of else branch
            {
                let else_result = if let Some(name) = scrut_name {
                    let mut narrowed_scope = Scope::with_parent(scope.clone());
                    narrowed_scope.define(name, narrowed_val.clone());
                    self.eval_node(else_branch, &mut narrowed_scope, exclude)
                } else {
                    self.eval_node(else_branch, scope, exclude)
                };
                match else_result {
                    Ok(else_val) => Self::check_branch_types(&matched, &else_val, _node)?,
                    Err(e) if e.is_runtime() => {}
                    Err(e) => return Err(e),
                }
            }
            // Speculative eval of non-matched when branches with narrowed scope
            for wc in when_clauses {
                let type_name = match &wc.value.kind {
                    NodeKind::Identifier { name } => name.as_str(),
                    _ => continue,
                };
                let branch_result = if let Some(name) = scrut_name {
                    let narrowed = self.create_narrowed_value_for_type(type_name, &narrowed_val);
                    let mut narrowed_scope = Scope::with_parent(scope.clone());
                    narrowed_scope.define(name, narrowed);
                    self.eval_node(&wc.result, &mut narrowed_scope, exclude)
                } else {
                    self.eval_node(&wc.result, scope, exclude)
                };
                match branch_result {
                    Ok(branch_val) => Self::check_branch_types(&matched, &branch_val, _node)?,
                    Err(e) if e.is_runtime() => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(matched)
        } else {
            // §5.10: else branch — narrow scrutinee to inner value.
            let else_val = if let Some(name) = scrut_name {
                let mut narrowed_scope = Scope::with_parent(scope.clone());
                narrowed_scope.define(name, narrowed_val.clone());
                self.eval_node(else_branch, &mut narrowed_scope, exclude)?
            } else {
                self.eval_node(else_branch, scope, exclude)?
            };
            // §D.5: speculatively evaluate when branches with narrowed scope.
            for wc in when_clauses {
                let type_name = match &wc.value.kind {
                    NodeKind::Identifier { name } => name.as_str(),
                    _ => continue,
                };
                let branch_result = if let Some(name) = scrut_name {
                    let narrowed = self.create_narrowed_value_for_type(type_name, &narrowed_val);
                    let mut narrowed_scope = Scope::with_parent(scope.clone());
                    narrowed_scope.define(name, narrowed);
                    self.eval_node(&wc.result, &mut narrowed_scope, exclude)
                } else {
                    self.eval_node(&wc.result, scope, exclude)
                };
                match branch_result {
                    Ok(branch_val) => Self::check_branch_types(&else_val, &branch_val, _node)?,
                    Err(e) if e.is_runtime() => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(else_val)
        }
    }

    fn eval_case_when_value(
        &mut self,
        wc: &WhenClause,
        scrut_val: &Value,
        matched_result: &mut Option<Value>,
        scope: &mut Scope,
        exclude: Option<&str>,
    ) -> Result<()> {
        let when_val = if let (Value::Enum(e), NodeKind::Identifier { name }) = (scrut_val, &wc.value.kind) {
            if e.variants.contains(name) {
                Value::Enum(UzonEnum::new(name.clone(), e.variants.clone(), e.type_name.clone()))
            } else {
                self.eval_node(&wc.value, scope, exclude)?
            }
        } else {
            self.eval_node(&wc.value, scope, exclude)?
        };

        // §5.10 v0.8: `when undefined` is a type error (not runtime)
        if when_val.is_undefined() {
            return Err(UzonError::type_error(
                "'when undefined' is not allowed; undefined is a state, not a matchable value",
                wc.value.span.line, wc.value.span.col,
            ));
        }

        if !scrut_val.is_null() && !when_val.is_null()
            && scrut_val.type_name() != when_val.type_name()
            && !can_adopt_cross_category(scrut_val, &when_val)
        {
            return Err(UzonError::type_error(
                format!(
                    "case scrutinee and when value must be the same type, got {} and {}",
                    scrut_val.type_name(), when_val.type_name()
                ),
                wc.value.span.line, wc.value.span.col,
            ));
        }

        if matched_result.is_none() && values_equal(scrut_val, &when_val) {
            *matched_result = Some(self.eval_node(&wc.result, scope, exclude)?);
        }

        Ok(())
    }

    /// §5.9/§D.5: Speculatively type-check all case value branches.
    /// RuntimeError is suppressed; TypeError is always propagated.
    fn typecheck_case_branches(
        &mut self,
        matched_result: Option<Value>,
        when_clauses: &[WhenClause],
        else_branch: &Node,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        if let Some(matched) = matched_result {
            let else_result = if let Value::Enum(ref e) = matched {
                self.resolve_enum_context(else_branch, e, scope, exclude)
            } else {
                self.eval_node(else_branch, scope, exclude)
            };
            match else_result {
                Ok(else_val) => Self::check_branch_types(&matched, &else_val, node)?,
                Err(e) if e.is_runtime() => {}
                Err(e) => return Err(e),
            }
            for wc in when_clauses {
                let branch_result = if let Value::Enum(ref e) = matched {
                    self.resolve_enum_context(&wc.result, e, scope, exclude)
                } else {
                    self.eval_node(&wc.result, scope, exclude)
                };
                match branch_result {
                    Ok(branch_val) => Self::check_branch_types(&matched, &branch_val, node)?,
                    Err(e) if e.is_runtime() => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(matched)
        } else {
            let else_val = self.eval_node(else_branch, scope, exclude)?;
            for wc in when_clauses {
                let branch_result = if let Value::Enum(ref e) = else_val {
                    self.resolve_enum_context(&wc.result, e, scope, exclude)
                } else {
                    self.eval_node(&wc.result, scope, exclude)
                };
                match branch_result {
                    Ok(branch_val) => Self::check_branch_types(&else_val, &branch_val, node)?,
                    Err(e) if e.is_runtime() => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(else_val)
        }
    }
}
