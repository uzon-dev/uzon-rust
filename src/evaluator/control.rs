// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::scope::Scope;
use crate::value::*;

use super::{Evaluator, values_equal};

impl Evaluator {
    /// Evaluate a case/when expression (§5.10, §5.9).
    pub(crate) fn eval_case(
        &mut self,
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

        // §11.2.1: cannot branch on untagged union values with case
        if matches!(&scrut_val, Value::Union(_)) {
            return Err(UzonError::type_error(
                "cannot branch on untagged union with 'case'; use tagged union instead",
                node.span.line, node.span.col,
            ));
        }

        // Phase 1: Find the matching clause
        let mut matched_result: Option<Value> = None;

        for wc in when_clauses {
            if wc.is_named {
                self.eval_case_when_named(wc, &scrut_val, &mut matched_result, scope, exclude, node)?;
            } else {
                self.eval_case_when_value(wc, &scrut_val, &mut matched_result, scope, exclude)?;
            }
        }

        // Phase 2: Type-check all branches speculatively
        self.typecheck_case_branches(matched_result, when_clauses, else_branch, scope, exclude, node)
    }

    fn eval_case_when_named(
        &mut self,
        wc: &WhenClause,
        scrut_val: &Value,
        matched_result: &mut Option<Value>,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<()> {
        if let NodeKind::Identifier { name } = &wc.value.kind {
            match scrut_val {
                Value::TaggedUnion(tu) => {
                    if !tu.variants.contains_key(name.as_str()) {
                        return Err(UzonError::type_error(
                            format!("'{}' is not a valid variant of this tagged union", name),
                            wc.value.span.line, wc.value.span.col,
                        ));
                    }
                    if matched_result.is_none() && tu.tag == *name {
                        *matched_result = Some(self.eval_node(&wc.result, scope, exclude)?);
                    }
                }
                _ => {
                    return Err(UzonError::type_error(
                        format!("'when named' requires tagged union scrutinee, got {}", scrut_val.type_name()),
                        node.span.line, node.span.col,
                    ));
                }
            }
        }
        Ok(())
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

        if when_val.is_undefined() {
            return Err(UzonError::runtime(
                "'when undefined' is not allowed; undefined is a state, not a matchable value",
                wc.value.span.line, wc.value.span.col,
            ));
        }

        if !scrut_val.is_null() && !when_val.is_null()
            && scrut_val.type_name() != when_val.type_name()
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

    /// §5.9: Speculatively type-check all case branches.
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
            if let Ok(else_val) = else_result {
                Self::check_branch_types(&matched, &else_val, node)?;
            }
            for wc in when_clauses {
                let branch_result = if let Value::Enum(ref e) = matched {
                    self.resolve_enum_context(&wc.result, e, scope, exclude)
                } else {
                    self.eval_node(&wc.result, scope, exclude)
                };
                if let Ok(branch_val) = branch_result {
                    Self::check_branch_types(&matched, &branch_val, node)?;
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
                if let Ok(branch_val) = branch_result {
                    Self::check_branch_types(&else_val, &branch_val, node)?;
                }
            }
            Ok(else_val)
        }
    }
}
