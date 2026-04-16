// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::scope::Scope;
use crate::value::*;

use super::Evaluator;

impl Evaluator {
    // === Dependency resolution (Kahn's algorithm) ===

    pub(crate) fn topological_sort(&self, bindings: &[Binding], scope: &Scope) -> Result<Vec<usize>> {
        let names: Vec<&str> = bindings.iter().map(|b| b.name.as_str()).collect();
        let name_to_idx: HashMap<&str, usize> = names.iter().enumerate().map(|(i, n)| (*n, i)).collect();

        let called_to_idx: HashMap<&str, usize> = bindings.iter().enumerate()
            .filter_map(|(i, b)| b.called.as_deref().map(|name| (name, i)))
            .collect();

        // Build adjacency: deps[i] = set of indices that binding i depends ON
        let mut deps: Vec<HashSet<usize>> = Vec::with_capacity(bindings.len());
        for (i, binding) in bindings.iter().enumerate() {
            let mut dep_set = HashSet::new();
            self.collect_deps(&binding.value, &names, &name_to_idx, &binding.name, scope, &mut dep_set);
            self.collect_type_deps(&binding.value, &called_to_idx, &mut dep_set);
            dep_set.remove(&i); // a binding does not depend on itself
            deps.push(dep_set);
        }

        // Build proper edges: if i depends on j, edge j → i
        let mut in_deg: Vec<usize> = vec![0; bindings.len()];
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); bindings.len()];
        for (i, dep_set) in deps.iter().enumerate() {
            in_deg[i] = dep_set.len();
            for &j in dep_set {
                dependents[j].push(i);
            }
        }

        let mut q: Vec<usize> = (0..bindings.len())
            .filter(|&i| in_deg[i] == 0)
            .collect();
        q.reverse(); // stable ordering: earlier bindings processed first
        let mut result = Vec::with_capacity(bindings.len());

        while let Some(node) = q.pop() {
            result.push(node);
            for &dep in &dependents[node] {
                in_deg[dep] -= 1;
                if in_deg[dep] == 0 {
                    q.push(dep);
                }
            }
        }

        if result.len() != bindings.len() {
            let in_cycle: Vec<&str> = (0..bindings.len())
                .filter(|&i| in_deg[i] > 0)
                .map(|i| names[i])
                .collect();
            return Err(UzonError::circular(
                format!("circular dependency among: {}", in_cycle.join(", ")),
                bindings[0].span.line,
                bindings[0].span.col,
            ));
        }

        Ok(result)
    }

    /// §3.8: Static check that function call graph is a DAG (no recursion).
    pub(crate) fn check_function_call_dag(&self, bindings: &[Binding]) -> Result<()> {
        let func_names: HashSet<&str> = bindings.iter()
            .filter(|b| matches!(b.value.kind, NodeKind::FunctionExpr { .. }))
            .map(|b| b.name.as_str())
            .collect();

        if func_names.is_empty() {
            return Ok(());
        }

        let mut call_graph: HashMap<&str, HashSet<&str>> = HashMap::new();
        for binding in bindings {
            if let NodeKind::FunctionExpr { body_bindings, body_expr, .. } = &binding.value.kind {
                let mut calls = HashSet::new();
                Self::collect_function_calls(body_expr, &func_names, &mut calls);
                for bb in body_bindings {
                    Self::collect_function_calls(&bb.value, &func_names, &mut calls);
                }
                call_graph.insert(binding.name.as_str(), calls);
            }
        }

        // DFS cycle detection (white=0, gray=1, black=2)
        let mut color: HashMap<&str, u8> = call_graph.keys().map(|&k| (k, 0u8)).collect();
        let mut cycle_path: Vec<&str> = Vec::new();

        for &name in call_graph.keys() {
            if color[name] == 0 {
                if self.dfs_find_cycle(name, &call_graph, &mut color, &mut cycle_path) {
                    let span = bindings.iter()
                        .find(|b| b.name == cycle_path[0])
                        .map(|b| b.span)
                        .unwrap_or(Span { line: 0, col: 0 });
                    return Err(UzonError::circular(
                        format!("recursive function call detected: {}", cycle_path.join(" → ")),
                        span.line, span.col,
                    ));
                }
            }
        }

        Ok(())
    }

    pub(crate) fn collect_function_calls<'a>(node: &'a Node, func_names: &HashSet<&str>, calls: &mut HashSet<&'a str>) {
        match &node.kind {
            NodeKind::FunctionCall { callee, args } => {
                if let NodeKind::Identifier { name } = &callee.kind {
                    if func_names.contains(name.as_str()) {
                        calls.insert(name.as_str());
                    }
                }
                Self::collect_function_calls(callee, func_names, calls);
                for arg in args {
                    Self::collect_function_calls(arg, func_names, calls);
                }
            }
            NodeKind::MemberAccess { object, .. } => {
                Self::collect_function_calls(object, func_names, calls);
            }
            NodeKind::BinaryOp { left, right, .. } => {
                Self::collect_function_calls(left, func_names, calls);
                Self::collect_function_calls(right, func_names, calls);
            }
            NodeKind::UnaryOp { operand, .. } => {
                Self::collect_function_calls(operand, func_names, calls);
            }
            NodeKind::IfExpr { condition, then_branch, else_branch } => {
                Self::collect_function_calls(condition, func_names, calls);
                Self::collect_function_calls(then_branch, func_names, calls);
                Self::collect_function_calls(else_branch, func_names, calls);
            }
            NodeKind::CaseExpr { scrutinee, when_clauses, else_branch, .. } => {
                Self::collect_function_calls(scrutinee, func_names, calls);
                for wc in when_clauses {
                    Self::collect_function_calls(&wc.value, func_names, calls);
                    Self::collect_function_calls(&wc.result, func_names, calls);
                }
                Self::collect_function_calls(else_branch, func_names, calls);
            }
            NodeKind::ListLiteral { elements } | NodeKind::TupleLiteral { elements } => {
                for elem in elements {
                    Self::collect_function_calls(elem, func_names, calls);
                }
            }
            NodeKind::StructLiteral { fields } => {
                for binding in fields {
                    Self::collect_function_calls(&binding.value, func_names, calls);
                }
            }
            NodeKind::StructOverride { base, overrides } | NodeKind::StructExtension { base, extension: overrides } => {
                Self::collect_function_calls(base, func_names, calls);
                Self::collect_function_calls(overrides, func_names, calls);
            }
            NodeKind::TypeAnnotation { expr, .. } | NodeKind::Conversion { expr, .. } => {
                Self::collect_function_calls(expr, func_names, calls);
            }
            NodeKind::StringLiteral { parts } => {
                for part in parts {
                    if let StringPart::Interpolation(expr) = part {
                        Self::collect_function_calls(expr, func_names, calls);
                    }
                }
            }
            NodeKind::FieldExtraction { source } => {
                Self::collect_function_calls(source, func_names, calls);
            }
            NodeKind::Grouping { expr } => {
                Self::collect_function_calls(expr, func_names, calls);
            }
            NodeKind::OrElse { left, right } => {
                Self::collect_function_calls(left, func_names, calls);
                Self::collect_function_calls(right, func_names, calls);
            }
            NodeKind::FromEnum { value, .. } | NodeKind::FromUnion { value, .. } | NodeKind::NamedVariant { value, .. } => {
                Self::collect_function_calls(value, func_names, calls);
            }
            NodeKind::FunctionExpr { body_bindings, body_expr, .. } => {
                Self::collect_function_calls(body_expr, func_names, calls);
                for bb in body_bindings {
                    Self::collect_function_calls(&bb.value, func_names, calls);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn dfs_find_cycle<'a>(
        &self,
        node: &'a str,
        graph: &HashMap<&'a str, HashSet<&'a str>>,
        color: &mut HashMap<&'a str, u8>,
        path: &mut Vec<&'a str>,
    ) -> bool {
        color.insert(node, 1);
        path.push(node);

        if let Some(neighbors) = graph.get(node) {
            for &neighbor in neighbors {
                match color.get(neighbor) {
                    Some(1) => {
                        path.push(neighbor);
                        return true;
                    }
                    Some(0) | None => {
                        if self.dfs_find_cycle(neighbor, graph, color, path) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }

        path.pop();
        color.insert(node, 2);
        false
    }

    /// Check if an expression is an env ref (e.g. `env.X`).
    pub(crate) fn expr_is_env_ref(node: &Node) -> bool {
        matches!(&node.kind, NodeKind::MemberAccess { object, .. } if matches!(object.kind, NodeKind::EnvRef))
    }

    /// §5.11 + §5.13: Check if string can be converted to the target type.
    pub(crate) fn check_string_conversion(target: &str, node: &Node) -> Result<()> {
        match target {
            t if IntegerType::from_type_name(t).is_some() => Ok(()),
            t if FloatType::from_type_name(t).is_some() => Ok(()),
            "string" => Ok(()),
            "bool" => Err(UzonError::type_error("cannot convert string to bool", node.span.line, node.span.col)),
            "null" => Err(UzonError::type_error("cannot convert string to null", node.span.line, node.span.col)),
            _ => Ok(()),
        }
    }


    pub(crate) fn collect_deps(
        &self,
        node: &Node,
        names: &[&str],
        name_to_idx: &HashMap<&str, usize>,
        _exclude: &str,
        _scope: &Scope,
        deps: &mut HashSet<usize>,
    ) {
        match &node.kind {
            NodeKind::MemberAccess { object, .. } => {
                self.collect_deps(object, names, name_to_idx, _exclude, _scope, deps);
            }
            NodeKind::BinaryOp { left, right, .. } => {
                self.collect_deps(left, names, name_to_idx, _exclude, _scope, deps);
                self.collect_deps(right, names, name_to_idx, _exclude, _scope, deps);
            }
            NodeKind::UnaryOp { operand, .. } => {
                self.collect_deps(operand, names, name_to_idx, _exclude, _scope, deps);
            }
            NodeKind::OrElse { left, right } => {
                self.collect_deps(left, names, name_to_idx, _exclude, _scope, deps);
                self.collect_deps(right, names, name_to_idx, _exclude, _scope, deps);
            }
            NodeKind::IfExpr { condition, then_branch, else_branch } => {
                self.collect_deps(condition, names, name_to_idx, _exclude, _scope, deps);
                self.collect_deps(then_branch, names, name_to_idx, _exclude, _scope, deps);
                self.collect_deps(else_branch, names, name_to_idx, _exclude, _scope, deps);
            }
            NodeKind::CaseExpr { scrutinee, when_clauses, else_branch, .. } => {
                self.collect_deps(scrutinee, names, name_to_idx, _exclude, _scope, deps);
                for wc in when_clauses {
                    self.collect_deps(&wc.value, names, name_to_idx, _exclude, _scope, deps);
                    self.collect_deps(&wc.result, names, name_to_idx, _exclude, _scope, deps);
                }
                self.collect_deps(else_branch, names, name_to_idx, _exclude, _scope, deps);
            }
            NodeKind::TypeAnnotation { expr, .. }
            | NodeKind::Conversion { expr, .. }
            | NodeKind::FromEnum { value: expr, .. }
            | NodeKind::FromUnion { value: expr, .. }
            | NodeKind::NamedVariant { value: expr, .. }
            | NodeKind::Grouping { expr }
            | NodeKind::FieldExtraction { source: expr } => {
                self.collect_deps(expr, names, name_to_idx, _exclude, _scope, deps);
            }
            NodeKind::StructOverride { base, overrides } => {
                self.collect_deps(base, names, name_to_idx, _exclude, _scope, deps);
                self.collect_deps(overrides, names, name_to_idx, _exclude, _scope, deps);
            }
            NodeKind::StructExtension { base, extension } => {
                self.collect_deps(base, names, name_to_idx, _exclude, _scope, deps);
                self.collect_deps(extension, names, name_to_idx, _exclude, _scope, deps);
            }
            NodeKind::FunctionExpr { body_bindings, body_expr, params, .. } => {
                for binding in body_bindings {
                    self.collect_deps(&binding.value, names, name_to_idx, _exclude, _scope, deps);
                }
                self.collect_deps(body_expr, names, name_to_idx, _exclude, _scope, deps);
                for param in params {
                    if let Some(ref default) = param.default {
                        self.collect_deps(default, names, name_to_idx, _exclude, _scope, deps);
                    }
                }
            }
            NodeKind::FunctionCall { callee, args } => {
                self.collect_deps(callee, names, name_to_idx, _exclude, _scope, deps);
                for arg in args {
                    self.collect_deps(arg, names, name_to_idx, _exclude, _scope, deps);
                }
            }
            NodeKind::StructLiteral { fields } => {
                for field in fields {
                    self.collect_deps(&field.value, names, name_to_idx, _exclude, _scope, deps);
                }
            }
            NodeKind::ListLiteral { elements } | NodeKind::TupleLiteral { elements } => {
                for elem in elements {
                    self.collect_deps(elem, names, name_to_idx, _exclude, _scope, deps);
                }
            }
            NodeKind::StringLiteral { parts } => {
                for part in parts {
                    if let StringPart::Interpolation(expr) = part {
                        self.collect_deps(expr, names, name_to_idx, _exclude, _scope, deps);
                    }
                }
            }
            NodeKind::Identifier { name } => {
                if let Some(&idx) = name_to_idx.get(name.as_str()) {
                    deps.insert(idx);
                }
            }
            _ => {}
        }
    }

    /// Collect dependencies on `called` type names from `as`/`to` type expressions.
    pub(crate) fn collect_type_deps(
        &self,
        node: &Node,
        called_to_idx: &HashMap<&str, usize>,
        deps: &mut HashSet<usize>,
    ) {
        match &node.kind {
            NodeKind::TypeAnnotation { expr, type_expr } | NodeKind::Conversion { expr, type_expr } => {
                if let Some(first) = type_expr.path.first() {
                    if let Some(&idx) = called_to_idx.get(first.as_str()) {
                        deps.insert(idx);
                    }
                }
                if let Some(ref inner) = type_expr.inner {
                    if let Some(first) = inner.path.first() {
                        if let Some(&idx) = called_to_idx.get(first.as_str()) {
                            deps.insert(idx);
                        }
                    }
                }
                self.collect_type_deps(expr, called_to_idx, deps);
            }
            NodeKind::BinaryOp { left, right, .. } | NodeKind::OrElse { left, right } => {
                self.collect_type_deps(left, called_to_idx, deps);
                self.collect_type_deps(right, called_to_idx, deps);
            }
            NodeKind::UnaryOp { operand, .. } | NodeKind::Grouping { expr: operand }
            | NodeKind::FromEnum { value: operand, .. }
            | NodeKind::FromUnion { value: operand, .. }
            | NodeKind::NamedVariant { value: operand, .. }
            | NodeKind::FieldExtraction { source: operand } => {
                self.collect_type_deps(operand, called_to_idx, deps);
            }
            NodeKind::IfExpr { condition, then_branch, else_branch } => {
                self.collect_type_deps(condition, called_to_idx, deps);
                self.collect_type_deps(then_branch, called_to_idx, deps);
                self.collect_type_deps(else_branch, called_to_idx, deps);
            }
            NodeKind::CaseExpr { scrutinee, when_clauses, else_branch, .. } => {
                self.collect_type_deps(scrutinee, called_to_idx, deps);
                for wc in when_clauses {
                    self.collect_type_deps(&wc.value, called_to_idx, deps);
                    self.collect_type_deps(&wc.result, called_to_idx, deps);
                }
                self.collect_type_deps(else_branch, called_to_idx, deps);
            }
            NodeKind::MemberAccess { object, .. } => {
                self.collect_type_deps(object, called_to_idx, deps);
            }
            NodeKind::StructOverride { base, overrides } => {
                self.collect_type_deps(base, called_to_idx, deps);
                self.collect_type_deps(overrides, called_to_idx, deps);
            }
            NodeKind::StructExtension { base, extension } => {
                self.collect_type_deps(base, called_to_idx, deps);
                self.collect_type_deps(extension, called_to_idx, deps);
            }
            NodeKind::FunctionExpr { body_bindings, body_expr, params, .. } => {
                for binding in body_bindings {
                    self.collect_type_deps(&binding.value, called_to_idx, deps);
                }
                self.collect_type_deps(body_expr, called_to_idx, deps);
                for param in params {
                    if let Some(ref default) = param.default {
                        self.collect_type_deps(default, called_to_idx, deps);
                    }
                }
            }
            NodeKind::FunctionCall { callee, args } => {
                self.collect_type_deps(callee, called_to_idx, deps);
                for arg in args {
                    self.collect_type_deps(arg, called_to_idx, deps);
                }
            }
            NodeKind::StructLiteral { fields } => {
                for field in fields {
                    self.collect_type_deps(&field.value, called_to_idx, deps);
                }
            }
            NodeKind::ListLiteral { elements } | NodeKind::TupleLiteral { elements } => {
                for elem in elements {
                    self.collect_type_deps(elem, called_to_idx, deps);
                }
            }
            NodeKind::StringLiteral { parts } => {
                for part in parts {
                    if let StringPart::Interpolation(expr) = part {
                        self.collect_type_deps(expr, called_to_idx, deps);
                    }
                }
            }
            _ => {}
        }
    }
}
