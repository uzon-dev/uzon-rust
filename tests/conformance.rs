// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Conformance test runner for the UZON specification.
//!
//! Exercises four test categories under `../conformance/`:
//!
//! | Category         | Assertion                                              |
//! |------------------|--------------------------------------------------------|
//! | `parse/valid`    | Must lex + parse without error                         |
//! | `parse/invalid`  | Must fail at lex, parse, or eval                       |
//! | `eval`           | Evaluated bindings must match `.expected.uzon` sibling  |
//! | `roundtrip`      | stringify → re-eval → re-stringify must be stable       |

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use uzon::lexer::Lexer;
use uzon::parser::Parser;
use uzon::{from_str, to_string, Value};

// ─── helpers ─────────────────────────────────────────────────────────────────

/// Root of the conformance suite (sibling directory to the crate root).
fn suite_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate must live inside a workspace")
        .join("conformance")
}

/// Recursively collect all `.uzon` files under `dir`, sorted for determinism.
fn collect_uzon(dir: &Path) -> Vec<PathBuf> {
    let mut acc = Vec::new();
    gather(dir, &mut acc);
    acc.sort();
    acc
}

fn gather(dir: &Path, acc: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    // Multi-file fixture convention: when a directory contains `entry.uzon`,
    // only that file is a test; sibling `.uzon` files are helper modules.
    let has_entry = std::fs::read_dir(dir)
        .map(|rd2| {
            rd2.flatten().any(|e| {
                e.path().file_name().and_then(|n| n.to_str()) == Some("entry.uzon")
            })
        })
        .unwrap_or(false);
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            gather(&p, acc);
        } else if p.extension().is_some_and(|e| e == "uzon") {
            if has_entry && p.file_name().and_then(|n| n.to_str()) != Some("entry.uzon") {
                continue;
            }
            acc.push(p);
        }
    }
}

/// Normalize for comparison: trim trailing whitespace per line, unify line
/// endings, strip trailing newline.
fn norm(s: &str) -> String {
    s.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end_matches('\n')
        .to_string()
}

/// Stringify a single binding so we can compare individual keys.
fn stringify_one(key: &str, val: &Value) -> String {
    let mut map = BTreeMap::new();
    map.insert(key.to_string(), val.clone());
    to_string(&map)
}

// ─── parse/valid ─────────────────────────────────────────────────────────────

#[test]
fn conformance_parse_valid() {
    let base = suite_root().join("parse/valid");
    let files = collect_uzon(&base);
    assert!(!files.is_empty(), "no parse/valid fixtures found");

    let mut ok = 0usize;
    let mut errs: Vec<String> = Vec::new();

    for f in &files {
        let src = std::fs::read_to_string(f).unwrap();
        let res = Lexer::new(&src)
            .tokenize()
            .and_then(|(toks, cl)| Parser::new(toks, cl).parse().map(drop));
        match res {
            Ok(()) => ok += 1,
            Err(e) => errs.push(format!(
                "  FAIL {}: {e}",
                f.strip_prefix(&base).unwrap_or(f).display()
            )),
        }
    }

    if !errs.is_empty() {
        panic!(
            "parse/valid: {ok}/{} passed\n{}",
            files.len(),
            errs.join("\n")
        );
    }
    eprintln!("parse/valid: {ok}/{} passed", files.len());
}

// ─── parse/invalid ───────────────────────────────────────────────────────────

#[test]
fn conformance_parse_invalid() {
    let base = suite_root().join("parse/invalid");
    let files = collect_uzon(&base);
    assert!(!files.is_empty(), "no parse/invalid fixtures found");

    let mut ok = 0usize;
    let mut errs: Vec<String> = Vec::new();

    for f in &files {
        let src = std::fs::read_to_string(f).unwrap();
        match from_str(&src) {
            Err(_) => ok += 1,
            Ok(_) => errs.push(format!(
                "  FAIL (expected error) {}",
                f.strip_prefix(&base).unwrap_or(f).display()
            )),
        }
    }

    if !errs.is_empty() {
        panic!(
            "parse/invalid: {ok}/{} passed\n{}",
            files.len(),
            errs.join("\n")
        );
    }
    eprintln!("parse/invalid: {ok}/{} passed", files.len());
}

// ─── eval ────────────────────────────────────────────────────────────────────

#[test]
fn conformance_eval() {
    let base = suite_root().join("eval");

    // Input files are those without `.expected.` in the name.
    let inputs: Vec<PathBuf> = collect_uzon(&base)
        .into_iter()
        .filter(|p| {
            !p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains(".expected.")
        })
        .collect();
    assert!(!inputs.is_empty(), "no eval fixtures found");

    let mut ok = 0usize;
    let mut skipped = 0usize;
    let mut errs: Vec<String> = Vec::new();

    for f in &inputs {
        let exp_path = f.with_extension("expected.uzon");
        if !exp_path.exists() {
            skipped += 1;
            continue;
        }

        let src = std::fs::read_to_string(f).unwrap();
        let exp_src = std::fs::read_to_string(&exp_path).unwrap();

        let actual = match from_str(&src) {
            Ok(v) => v,
            Err(e) => {
                errs.push(format!(
                    "  FAIL {}: eval error: {e}",
                    f.strip_prefix(&base).unwrap_or(f).display()
                ));
                continue;
            }
        };

        let expected = match from_str(&exp_src) {
            Ok(v) => v,
            Err(e) => {
                errs.push(format!(
                    "  FAIL {}: expected-file eval error: {e}",
                    f.strip_prefix(&base).unwrap_or(f).display()
                ));
                continue;
            }
        };

        // Only compare bindings present in the expected file (may be a subset).
        let mut diffs: Vec<String> = Vec::new();
        for (key, exp_val) in &expected {
            match actual.get(key) {
                None => diffs.push(format!("    binding '{key}': missing from output")),
                Some(act_val) => {
                    let a = norm(&stringify_one(key, act_val));
                    let e = norm(&stringify_one(key, exp_val));
                    if a != e {
                        diffs.push(format!(
                            "    binding '{key}':\n      expected: {}\n      actual:   {}",
                            e.trim(),
                            a.trim(),
                        ));
                    }
                }
            }
        }

        if diffs.is_empty() {
            ok += 1;
        } else {
            errs.push(format!(
                "  FAIL {}:\n{}",
                f.strip_prefix(&base).unwrap_or(f).display(),
                diffs.join("\n"),
            ));
        }
    }

    let total = inputs.len() - skipped;
    if !errs.is_empty() {
        panic!(
            "eval: {ok}/{total} passed ({skipped} skipped)\n{}",
            errs.join("\n\n")
        );
    }
    eprintln!("eval: {ok}/{total} passed ({skipped} skipped)");
}

// ─── roundtrip ───────────────────────────────────────────────────────────────

#[test]
fn conformance_roundtrip() {
    let base = suite_root().join("roundtrip");
    let files = collect_uzon(&base);
    assert!(!files.is_empty(), "no roundtrip fixtures found");

    let mut ok = 0usize;
    let mut errs: Vec<String> = Vec::new();

    for f in &files {
        let src = std::fs::read_to_string(f).unwrap();
        let rel = f.strip_prefix(&base).unwrap_or(f).display().to_string();

        // phase 1: eval
        let v1 = match from_str(&src) {
            Ok(v) => v,
            Err(e) => {
                errs.push(format!("  FAIL {rel}: phase-1 eval: {e}"));
                continue;
            }
        };

        // phase 2: stringify
        let s1 = to_string(&v1);

        // phase 3: re-eval the stringified form
        let v2 = match from_str(&s1) {
            Ok(v) => v,
            Err(e) => {
                errs.push(format!(
                    "  FAIL {rel}: phase-3 re-eval: {e}\n    --- stringified ---\n    {s1}"
                ));
                continue;
            }
        };

        // phase 4: re-stringify and compare
        let n1 = norm(&to_string(&v1));
        let n2 = norm(&to_string(&v2));

        if n1 != n2 {
            let l1: Vec<&str> = n1.lines().collect();
            let l2: Vec<&str> = n2.lines().collect();
            let detail = l1
                .iter()
                .zip(l2.iter())
                .enumerate()
                .find(|(_, (a, b))| a != b)
                .map(|(i, (a, b))| {
                    format!(
                        "line {}:\n    original:  {a}\n    roundtrip: {b}",
                        i + 1
                    )
                })
                .unwrap_or_else(|| {
                    format!("length: {} vs {} lines", l1.len(), l2.len())
                });
            errs.push(format!("  FAIL {rel}: mismatch at {detail}"));
        } else {
            ok += 1;
        }
    }

    if !errs.is_empty() {
        panic!(
            "roundtrip: {ok}/{} passed\n{}",
            files.len(),
            errs.join("\n\n")
        );
    }
    eprintln!("roundtrip: {ok}/{} passed", files.len());
}
