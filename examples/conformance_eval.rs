use std::collections::BTreeMap;
use std::path::Path;

fn main() {
    let base = "/home/suho/code/uzon-dev/conformance/eval";
    let mut pass = 0;
    let mut fail = 0;
    let mut failures = Vec::new();

    // Collect all .uzon files that are NOT .expected.uzon
    let mut test_files: Vec<String> = std::fs::read_dir(base)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|f| f.ends_with(".uzon") && !f.ends_with(".expected.uzon"))
        .collect();
    test_files.sort();

    for file in &test_files {
        let input_path = format!("{}/{}", base, file);
        let stem = file.strip_suffix(".uzon").unwrap();
        let expected_path = format!("{}/{}.expected.uzon", base, stem);

        if !Path::new(&expected_path).exists() {
            failures.push(format!("  SKIP {}: no expected file", file));
            fail += 1;
            continue;
        }

        // Evaluate input
        let input_result = uzon::from_str_plain(
            &std::fs::read_to_string(&input_path).unwrap()
        );
        let expected_result = uzon::from_str_plain(
            &std::fs::read_to_string(&expected_path).unwrap()
        );

        match (input_result, expected_result) {
            (Ok(actual), Ok(expected)) => {
                if values_equal_map(&actual, &expected) {
                    pass += 1;
                } else {
                    fail += 1;
                    let diff = find_diff(&actual, &expected);
                    failures.push(format!("  FAIL {}: {}", file, diff));
                }
            }
            (Err(e), _) => {
                fail += 1;
                failures.push(format!("  FAIL {}: input eval error: {}", file, e));
            }
            (_, Err(e)) => {
                fail += 1;
                failures.push(format!("  FAIL {}: expected eval error: {}", file, e));
            }
        }
    }

    println!("=== Eval conformance: {}/{} passed ===", pass, pass + fail);
    for f in &failures {
        println!("{}", f);
    }
    if fail > 0 {
        std::process::exit(1);
    }
}

/// Compare only keys present in expected (expected may intentionally omit
/// function bindings, undefined values, and underscore-prefixed helpers).
fn values_equal_map(actual: &BTreeMap<String, uzon::Value>, expected: &BTreeMap<String, uzon::Value>) -> bool {
    for (k, ev) in expected {
        match actual.get(k) {
            Some(av) => if !values_equal(av, ev) { return false; },
            None => return false,
        }
    }
    true
}

fn values_equal(a: &uzon::Value, b: &uzon::Value) -> bool {
    use uzon::Value::*;
    match (a, b) {
        (Null, Null) => true,
        (Bool(a), Bool(b)) => a == b,
        (Integer(a), Integer(b)) => a.value == b.value,
        (Float(a), Float(b)) => {
            if a.value.is_nan() && b.value.is_nan() { return true; }
            a.value == b.value
        }
        (String(a), String(b)) => a == b,
        (List(a), List(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| values_equal(x, y))
        }
        (Struct(a), Struct(b)) => {
            // For nested structs, require exact match (not subset)
            if a.len() != b.len() { return false; }
            a.iter().all(|(k, av)| b.get(k).is_some_and(|bv| values_equal(av, bv)))
        }
        (Undefined, Undefined) => true,
        _ => false,
    }
}

fn find_diff(actual: &BTreeMap<String, uzon::Value>, expected: &BTreeMap<String, uzon::Value>) -> String {
    for (k, ev) in expected {
        match actual.get(k) {
            None => return format!("missing key '{}'", k),
            Some(av) => {
                if !values_equal(av, ev) {
                    return format!("key '{}': expected {:?}, got {:?}", k, ev, av);
                }
            }
        }
    }
    for k in actual.keys() {
        if !expected.contains_key(k) {
            return format!("unexpected key '{}'", k);
        }
    }
    "unknown diff".to_string()
}
