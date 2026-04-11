use std::collections::BTreeMap;

fn main() {
    let base = "/home/suho/code/uzon-dev/conformance/roundtrip";
    let mut pass = 0;
    let mut fail = 0;
    let mut failures = Vec::new();

    let mut test_files: Vec<String> = std::fs::read_dir(base)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|f| f.ends_with(".uzon"))
        .collect();
    test_files.sort();

    for file in &test_files {
        let path = format!("{}/{}", base, file);
        let source = std::fs::read_to_string(&path).unwrap();

        // Step 1: parse + eval
        let values1 = match uzon::from_str(&source) {
            Ok(v) => v,
            Err(e) => {
                fail += 1;
                failures.push(format!("  FAIL {}: parse error: {}", file, e));
                continue;
            }
        };

        // Step 2: stringify
        let text = uzon::to_string(&values1);

        // Step 3: parse + eval the stringified output
        let values2 = match uzon::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                fail += 1;
                failures.push(format!(
                    "  FAIL {}: re-parse error: {}\n    stringified:\n{}",
                    file, e,
                    indent(&text),
                ));
                continue;
            }
        };

        // Step 4: compare (only keys present in values2; helpers/functions may be stripped)
        let mismatch = find_mismatch(&values1, &values2);
        if let Some(diff) = mismatch {
            fail += 1;
            failures.push(format!(
                "  FAIL {}: {}\n    stringified:\n{}",
                file, diff,
                indent(&text),
            ));
        } else {
            pass += 1;
        }
    }

    println!("=== Roundtrip conformance: {}/{} passed ===", pass, pass + fail);
    for f in &failures {
        println!("{}", f);
    }
    if fail > 0 {
        std::process::exit(1);
    }
}

fn indent(text: &str) -> String {
    text.lines().map(|l| format!("      {l}")).collect::<Vec<_>>().join("\n")
}

fn find_mismatch(
    original: &BTreeMap<String, uzon::Value>,
    roundtripped: &BTreeMap<String, uzon::Value>,
) -> Option<String> {
    // Check all keys from roundtripped exist in original with same value
    for (k, rv) in roundtripped {
        match original.get(k) {
            None => return Some(format!("extra key '{}' after roundtrip", k)),
            Some(ov) => {
                if !values_equal(ov, rv) {
                    return Some(format!(
                        "key '{}' changed:\n      before: {:?}\n      after:  {:?}",
                        k, ov, rv
                    ));
                }
            }
        }
    }
    // Check all non-function, non-undefined keys from original are in roundtripped
    for (k, ov) in original {
        if is_non_roundtrippable(ov) {
            continue;
        }
        if !roundtripped.contains_key(k) {
            return Some(format!("key '{}' lost after roundtrip", k));
        }
    }
    None
}

fn is_non_roundtrippable(v: &uzon::Value) -> bool {
    matches!(v, uzon::Value::Function(_) | uzon::Value::Undefined)
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
        (Tuple(a), Tuple(b)) => {
            a.elements.len() == b.elements.len()
                && a.elements.iter().zip(b.elements.iter()).all(|(x, y)| values_equal(x, y))
        }
        (Struct(a), Struct(b)) => {
            a.len() == b.len()
                && a.iter().all(|(k, av)| b.get(k).is_some_and(|bv| values_equal(av, bv)))
        }
        (Enum(a), Enum(b)) => {
            a.value == b.value && a.variants == b.variants && a.type_name == b.type_name
        }
        (Union(a), Union(b)) => {
            values_equal(&a.value, &b.value) && a.types == b.types && a.type_name == b.type_name
        }
        (TaggedUnion(a), TaggedUnion(b)) => {
            values_equal(&a.value, &b.value)
                && a.tag == b.tag
                && a.variants == b.variants
                && a.type_name == b.type_name
        }
        _ => false,
    }
}
