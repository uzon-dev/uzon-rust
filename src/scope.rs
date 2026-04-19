// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use indexmap::IndexMap;

use crate::value::Value;

/// Type information registered with `called` (§6.2).
#[derive(Debug, Clone)]
pub struct TypeDef {
    pub kind: TypeDefKind,
    pub name: String,
}

/// Field type information for named struct conformance checking (§6.3) and
/// v0.10 field defaults (§3.2).
#[derive(Debug, Clone)]
pub struct StructFieldInfo {
    /// Runtime type category (e.g., "integer", "float", "string", "bool", "struct").
    pub type_category: String,
    /// Type annotation if the field was defined with `as` (e.g., "i32", "f64", "Point").
    pub type_annotation: Option<String>,
    /// §3.2 v0.10: the field's declared value, used as the default when a
    /// value of this named struct type is constructed with the field omitted.
    pub default_value: Value,
}

#[derive(Debug, Clone)]
pub enum TypeDefKind {
    Enum {
        variants: Vec<String>,
    },
    Union {
        types: Vec<String>,
    },
    TaggedUnion {
        variants: BTreeMap<String, Option<String>>,
    },
    Struct {
        /// §11.1: fields are stored in declaration order.
        fields: IndexMap<String, StructFieldInfo>,
    },
    Function {
        param_types: Vec<String>,
        return_type: String,
    },
}

/// Lexical scope with parent chain for nested structs (§5.12).
///
/// The scope chain allows nested structs to access bindings and types from
/// enclosing scopes. The own-name exclusion rule (§5.12) prevents a binding
/// from seeing its own value during evaluation.
#[derive(Debug, Clone)]
pub struct Scope {
    bindings: BTreeMap<String, Value>,
    types: BTreeMap<String, TypeDef>,
    parent: Option<Box<Scope>>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            bindings: BTreeMap::new(),
            types: BTreeMap::new(),
            parent: None,
        }
    }

    pub fn with_parent(parent: Scope) -> Self {
        Self {
            bindings: BTreeMap::new(),
            types: BTreeMap::new(),
            parent: Some(Box::new(parent)),
        }
    }

    pub fn define(&mut self, name: impl Into<String>, value: Value) {
        self.bindings.insert(name.into(), value);
    }

    /// Look up a binding in the scope chain.
    ///
    /// `exclude`: if Some, skip this name in the current scope (own-name exclusion rule §5.12).
    /// The exclusion only applies to the current scope; parent lookups are unaffected.
    pub fn get(&self, name: &str, exclude: Option<&str>) -> Option<&Value> {
        if exclude != Some(name) {
            if let Some(v) = self.bindings.get(name) {
                return Some(v);
            }
        }
        if let Some(ref parent) = self.parent {
            parent.get(name, None)
        } else {
            None
        }
    }

    pub fn has(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    pub fn define_type(&mut self, name: impl Into<String>, typedef: TypeDef) {
        self.types.insert(name.into(), typedef);
    }

    /// Look up a type in the scope chain.
    pub fn get_type(&self, name: &str) -> Option<&TypeDef> {
        if let Some(td) = self.types.get(name) {
            return Some(td);
        }
        if let Some(ref parent) = self.parent {
            parent.get_type(name)
        } else {
            None
        }
    }

    pub fn names(&self) -> Vec<String> {
        self.bindings.keys().cloned().collect()
    }

    pub fn to_map(&self) -> BTreeMap<String, Value> {
        self.bindings.clone()
    }

    /// Get type definitions from this scope only (not parent).
    pub fn local_types(&self) -> BTreeMap<String, TypeDef> {
        self.types.clone()
    }

    /// Collect all type definitions visible from this scope (including parent chain).
    pub fn all_types(&self) -> BTreeMap<String, TypeDef> {
        let mut types = BTreeMap::new();
        if let Some(ref parent) = self.parent {
            types.extend(parent.all_types());
        }
        types.extend(self.types.clone());
        types
    }

    pub fn into_parent(self) -> Option<Scope> {
        self.parent.map(|b| *b)
    }

    /// Resolve a type from a multi-segment path (e.g., `Config.Port`).
    pub fn get_type_from_path(&self, path: &[String]) -> Option<&TypeDef> {
        if path.is_empty() {
            return None;
        }
        if path.len() == 1 {
            return self.get_type(&path[0]);
        }
        // Multi-segment path: first segment is a binding name
        if let Some(Value::Struct(_)) = self.get(&path[0], None) {
            self.get_type(path.last().unwrap())
        } else {
            None
        }
    }

    /// Resolve a type path, trying scope types first, then value-based resolution.
    pub fn resolve_type_path(&self, path: &[String]) -> Option<TypeDef> {
        if let Some(td) = self.get_type_from_path(path) {
            return Some(td.clone());
        }
        self.resolve_type_from_value_path(path)
    }

    /// Resolve a type from a multi-segment path by traversing struct values.
    ///
    /// Handles types defined inside nested/imported structs
    /// (e.g., `_shared.base_module.ModulePurpose`).
    pub fn resolve_type_from_value_path(&self, path: &[String]) -> Option<TypeDef> {
        if path.len() < 2 {
            return None;
        }
        let type_name = path.last().unwrap();

        // Navigate through struct bindings: path[0] → ... → path[n-2]
        let mut current: Option<&Value> = self.get(&path[0], None);
        for segment in &path[1..path.len() - 1] {
            current = match current {
                Some(Value::Struct(m)) => m.get(segment),
                _ => return None,
            };
        }

        // Search the container struct for a value whose type_name matches
        let container = match current {
            Some(Value::Struct(m)) => m,
            _ => return None,
        };

        for val in container.values() {
            match val {
                Value::Enum(e) if e.type_name.as_deref() == Some(type_name.as_str()) => {
                    return Some(TypeDef {
                        name: type_name.clone(),
                        kind: TypeDefKind::Enum {
                            variants: e.variants.clone(),
                        },
                    });
                }
                Value::TaggedUnion(tu)
                    if tu.type_name.as_deref() == Some(type_name.as_str()) =>
                {
                    return Some(TypeDef {
                        name: type_name.clone(),
                        kind: TypeDefKind::TaggedUnion {
                            variants: tu.variants.clone(),
                        },
                    });
                }
                Value::Union(u) if u.type_name.as_deref() == Some(type_name.as_str()) => {
                    return Some(TypeDef {
                        name: type_name.clone(),
                        kind: TypeDefKind::Union {
                            types: u.types.clone(),
                        },
                    });
                }
                _ => {}
            }
        }

        None
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}
