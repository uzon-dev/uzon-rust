// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::scope::Scope;
use crate::value::*;

use super::Evaluator;

/// A recursive function call: the calling function's name and the call site location.
pub(crate) struct RecursiveCall {
    pub name: String,
    pub call_span: Span,
}

impl Evaluator {
    /// Unwrap TypeAnnotation / Conversion wrappers to reach the underlying
    /// FunctionExpr. Returns `(params, return_type, body_bindings, body_expr)`
    /// or `None` if the node is not (or does not wrap) a function expression.
    pub(crate) fn function_expr(node: &Node) -> Option<(&[FunctionParam], &TypeExpr, &[Binding], &Node)> {
        match &node.kind {
            NodeKind::FunctionExpr { params, return_type, body_bindings, body_expr } => {
                Some((params, return_type, body_bindings, body_expr))
            }
            NodeKind::TypeAnnotation { expr, .. }
            | NodeKind::Conversion { expr, .. }
            | NodeKind::Grouping { expr } => Self::function_expr(expr),
            _ => None,
        }
    }

    // === Dependency resolution (Kahn's algorithm) ===

    /// Returns `(order, cycle_indices)` where `order` contains indices of non-cycle
    /// bindings in evaluation order, and `cycle_indices` lists bindings in cycles.
    pub(crate) fn topological_sort(&self, bindings: &[Binding], scope: &Scope) -> (Vec<usize>, Vec<usize>) {
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

        let cycle_indices: Vec<usize> = (0..bindings.len())
            .filter(|&i| in_deg[i] > 0)
            .collect();

        (result, cycle_indices)
    }

    /// §3.8: Static check that function call graph is a DAG (no recursion).
    /// Returns recursive call sites with their spans (empty if no cycles).
    ///
    /// Call edges are conservative: when a function calls a parameter whose
    /// type is a named function type, the call graph includes edges to every
    /// function in the document whose signature matches that type.
    pub(crate) fn check_function_call_dag(&self, bindings: &[Binding]) -> Vec<RecursiveCall> {
        let func_names: HashSet<&str> = bindings.iter()
            .filter(|b| Self::function_expr(&b.value).is_some())
            .map(|b| b.name.as_str())
            .collect();

        if func_names.is_empty() {
            return Vec::new();
        }

        // Signature of each function binding, keyed by the binding's name.
        // Signature is (param type names in order, return type name).
        let mut signatures: HashMap<&str, (Vec<String>, String)> = HashMap::new();
        for b in bindings {
            if let Some((params, return_type, _, _)) = Self::function_expr(&b.value) {
                let sig = (
                    params.iter()
                        .map(|p| p.type_expr.path.last().cloned().unwrap_or_default())
                        .collect(),
                    return_type.path.last().cloned().unwrap_or_default(),
                );
                signatures.insert(b.name.as_str(), sig);
            }
        }

        // Named function types (`called T` on a function binding) map to their
        // signatures. Used to decide whether a parameter's declared type is a
        // function type, and if so, what signature it requires.
        let mut named_func_types: HashMap<&str, (Vec<String>, String)> = HashMap::new();
        for b in bindings {
            if let Some(ref called) = b.called {
                if let Some(sig) = signatures.get(b.name.as_str()) {
                    named_func_types.insert(called.as_str(), sig.clone());
                }
            }
        }

        // Group top-level function bindings by their structural signature.
        let mut by_sig: HashMap<(Vec<String>, String), Vec<&str>> = HashMap::new();
        for (&name, sig) in &signatures {
            by_sig.entry(sig.clone()).or_default().push(name);
        }

        let mut call_graph: HashMap<&str, HashSet<&str>> = HashMap::new();
        let mut call_spans: HashMap<&str, HashMap<&str, Span>> = HashMap::new();
        for binding in bindings {
            if let Some((params, _, body_bindings, body_expr)) = Self::function_expr(&binding.value) {
                let mut calls = HashSet::new();
                let mut spans = HashMap::new();

                // Parameters with function type: map param name → the set of
                // top-level functions whose signature matches the param's type.
                let mut param_targets: HashMap<&str, Vec<&str>> = HashMap::new();
                for p in params {
                    let Some(last) = p.type_expr.path.last() else { continue };
                    let Some(sig) = named_func_types.get(last.as_str()) else { continue };
                    if let Some(targets) = by_sig.get(sig) {
                        let filtered: Vec<&str> = targets.iter()
                            .copied()
                            .filter(|n| *n != binding.name.as_str())
                            .collect();
                        if !filtered.is_empty() {
                            param_targets.insert(p.name.as_str(), filtered);
                        }
                    }
                }

                Self::collect_function_calls_ext(
                    body_expr, &func_names, &param_targets, &mut calls, &mut spans,
                );
                for bb in body_bindings {
                    Self::collect_function_calls_ext(
                        &bb.value, &func_names, &param_targets, &mut calls, &mut spans,
                    );
                }
                call_graph.insert(binding.name.as_str(), calls);
                call_spans.insert(binding.name.as_str(), spans);
            }
        }

        // DFS cycle detection (white=0, gray=1, black=2)
        let mut color: HashMap<&str, u8> = call_graph.keys().map(|&k| (k, 0u8)).collect();
        let mut result = Vec::new();

        for &name in call_graph.keys() {
            if color[name] == 0 {
                let mut path = Vec::new();
                if self.dfs_find_cycle(name, &call_graph, &mut color, &mut path) {
                    // Collect all gray (cycle-participating) nodes
                    let gray_names: Vec<&str> = color.iter()
                        .filter(|&(_, &c)| c == 1)
                        .map(|(&k, _)| k)
                        .collect();
                    let gray_set: HashSet<&str> = gray_names.iter().copied().collect();

                    // Find call sites where a gray node calls another gray node
                    for &caller in &gray_names {
                        if let Some(spans) = call_spans.get(caller) {
                            if let Some(callees) = call_graph.get(caller) {
                                for &callee in callees {
                                    if gray_set.contains(callee) {
                                        if let Some(&span) = spans.get(callee) {
                                            result.push(RecursiveCall {
                                                name: caller.to_string(),
                                                call_span: span,
                                            });
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Mark gray nodes as black to avoid re-reporting
                    for (_, c) in color.iter_mut() {
                        if *c == 1 { *c = 2; }
                    }
                }
            }
        }

        result
    }

    /// §3.8 conservative higher-order edges: when the callee is a parameter
    /// whose type is a named function
    /// type, add edges to every top-level function whose signature matches.
    pub(crate) fn collect_function_calls_ext<'a>(
        node: &'a Node,
        func_names: &HashSet<&str>,
        param_targets: &HashMap<&str, Vec<&'a str>>,
        calls: &mut HashSet<&'a str>,
        spans: &mut HashMap<&'a str, Span>,
    ) {
        match &node.kind {
            NodeKind::FunctionCall { callee, args } => {
                if let NodeKind::Identifier { name } = &callee.kind {
                    if func_names.contains(name.as_str()) {
                        calls.insert(name.as_str());
                        spans.entry(name.as_str()).or_insert(node.span);
                    } else if let Some(targets) = param_targets.get(name.as_str()) {
                        for &t in targets {
                            calls.insert(t);
                            spans.entry(t).or_insert(node.span);
                        }
                    }
                }
                Self::collect_function_calls_ext(callee, func_names, param_targets, calls, spans);
                for arg in args {
                    Self::collect_function_calls_ext(arg, func_names, param_targets, calls, spans);
                }
            }
            NodeKind::MemberAccess { object, .. } => {
                Self::collect_function_calls_ext(object, func_names, param_targets, calls, spans);
            }
            NodeKind::BinaryOp { left, right, .. } => {
                Self::collect_function_calls_ext(left, func_names, param_targets, calls, spans);
                Self::collect_function_calls_ext(right, func_names, param_targets, calls, spans);
            }
            NodeKind::UnaryOp { operand, .. } => {
                Self::collect_function_calls_ext(operand, func_names, param_targets, calls, spans);
            }
            NodeKind::IfExpr { condition, then_branch, else_branch } => {
                Self::collect_function_calls_ext(condition, func_names, param_targets, calls, spans);
                Self::collect_function_calls_ext(then_branch, func_names, param_targets, calls, spans);
                Self::collect_function_calls_ext(else_branch, func_names, param_targets, calls, spans);
            }
            NodeKind::CaseExpr { scrutinee, when_clauses, else_branch, .. } => {
                Self::collect_function_calls_ext(scrutinee, func_names, param_targets, calls, spans);
                for wc in when_clauses {
                    Self::collect_function_calls_ext(&wc.value, func_names, param_targets, calls, spans);
                    Self::collect_function_calls_ext(&wc.result, func_names, param_targets, calls, spans);
                }
                Self::collect_function_calls_ext(else_branch, func_names, param_targets, calls, spans);
            }
            NodeKind::ListLiteral { elements } | NodeKind::TupleLiteral { elements } => {
                for elem in elements {
                    Self::collect_function_calls_ext(elem, func_names, param_targets, calls, spans);
                }
            }
            NodeKind::StructLiteral { fields } => {
                for binding in fields {
                    Self::collect_function_calls_ext(&binding.value, func_names, param_targets, calls, spans);
                }
            }
            NodeKind::StructOverride { base, overrides } | NodeKind::StructExtension { base, extension: overrides } => {
                Self::collect_function_calls_ext(base, func_names, param_targets, calls, spans);
                Self::collect_function_calls_ext(overrides, func_names, param_targets, calls, spans);
            }
            NodeKind::TypeAnnotation { expr, .. } | NodeKind::Conversion { expr, .. } => {
                Self::collect_function_calls_ext(expr, func_names, param_targets, calls, spans);
            }
            NodeKind::StringLiteral { parts } => {
                for part in parts {
                    if let StringPart::Interpolation(expr) = part {
                        Self::collect_function_calls_ext(expr, func_names, param_targets, calls, spans);
                    }
                }
            }
            NodeKind::FieldExtraction { source } => {
                Self::collect_function_calls_ext(source, func_names, param_targets, calls, spans);
            }
            NodeKind::Grouping { expr } => {
                Self::collect_function_calls_ext(expr, func_names, param_targets, calls, spans);
            }
            NodeKind::OrElse { left, right } => {
                Self::collect_function_calls_ext(left, func_names, param_targets, calls, spans);
                Self::collect_function_calls_ext(right, func_names, param_targets, calls, spans);
            }
            NodeKind::FromEnum { value, .. } | NodeKind::FromUnion { value, .. } | NodeKind::NamedVariant { value, .. } => {
                Self::collect_function_calls_ext(value, func_names, param_targets, calls, spans);
            }
            NodeKind::VariantShorthand { inner, .. } => {
                Self::collect_function_calls_ext(inner, func_names, param_targets, calls, spans);
            }
            NodeKind::FunctionExpr { body_bindings, body_expr, .. } => {
                Self::collect_function_calls_ext(body_expr, func_names, param_targets, calls, spans);
                for bb in body_bindings {
                    Self::collect_function_calls_ext(&bb.value, func_names, param_targets, calls, spans);
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
            NodeKind::TypeAnnotation { expr, type_expr }
            | NodeKind::Conversion { expr, type_expr } => {
                self.collect_deps(expr, names, name_to_idx, _exclude, _scope, deps);
                // §7.3: dotted type paths (e.g., `as module_b.Point`) depend
                // on the outer binding `module_b`, so the module binding must
                // evaluate before this annotation can resolve.
                if let Some(first) = type_expr.path.first() {
                    if let Some(&idx) = name_to_idx.get(first.as_str()) {
                        deps.insert(idx);
                    }
                }
                if let Some(ref inner) = type_expr.inner {
                    if let Some(first) = inner.path.first() {
                        if let Some(&idx) = name_to_idx.get(first.as_str()) {
                            deps.insert(idx);
                        }
                    }
                }
            }
            NodeKind::FromEnum { value: expr, .. }
            | NodeKind::FromUnion { value: expr, .. }
            | NodeKind::NamedVariant { value: expr, .. }
            | NodeKind::Grouping { expr }
            | NodeKind::FieldExtraction { source: expr } => {
                self.collect_deps(expr, names, name_to_idx, _exclude, _scope, deps);
            }
            NodeKind::VariantShorthand { inner, .. } => {
                self.collect_deps(inner, names, name_to_idx, _exclude, _scope, deps);
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
