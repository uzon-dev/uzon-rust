fn main() {
    let base = "/home/suho/code/uzon-dev/conformance/parse";
    let mut pass = 0;
    let mut fail = 0;
    let mut failures = Vec::new();

    // Collect valid .uzon files recursively
    let valid_files = collect_uzon_files(&format!("{}/valid", base));
    for path in &valid_files {
        // Use from_path for files with imports (resolves relative paths correctly)
        match uzon::from_path(std::path::Path::new(path)) {
            Ok(_) => pass += 1,
            Err(e) => {
                fail += 1;
                let name = path.strip_prefix(base).unwrap_or(path);
                failures.push(format!("  FAIL (should parse) {}: {}", name, e));
            }
        }
    }

    // Collect invalid .uzon files recursively. For multi-file fixtures
    // (subdirectory containing `entry.uzon`), only the entry is expected to
    // reject — helper modules are valid on their own and get imported via
    // `entry.uzon`. Use `from_path` so cross-file imports resolve.
    let invalid_root = format!("{}/invalid", base);
    let invalid_files = collect_uzon_files(&invalid_root);
    for path in &invalid_files {
        if is_import_helper(path, &invalid_root) {
            continue;
        }
        match uzon::from_path(std::path::Path::new(path)) {
            Err(_) => pass += 1,
            Ok(_) => {
                fail += 1;
                let name = path.strip_prefix(base).unwrap_or(path);
                failures.push(format!("  FAIL (should reject) {}", name));
            }
        }
    }

    let invalid_tested = invalid_files.iter().filter(|p| !is_import_helper(p, &invalid_root)).count();
    println!(
        "=== Parse conformance: {}/{} passed (valid: {}, invalid: {}) ===",
        pass,
        pass + fail,
        valid_files.len(),
        invalid_tested
    );
    for f in &failures {
        println!("{}", f);
    }
    if fail > 0 {
        std::process::exit(1);
    }
}

/// Multi-file invalid fixtures use a subdirectory with `entry.uzon` as the
/// driver; sibling files are imported helpers that are valid on their own.
fn is_import_helper(path: &str, invalid_root: &str) -> bool {
    let rel = match path.strip_prefix(invalid_root) {
        Some(r) => r.trim_start_matches('/'),
        None => return false,
    };
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.len() <= 1 {
        return false;
    }
    let filename = parts.last().copied().unwrap_or("");
    let dir = std::path::Path::new(invalid_root).join(parts[..parts.len() - 1].join("/"));
    dir.join("entry.uzon").exists() && filename != "entry.uzon"
}

fn collect_uzon_files(dir: &str) -> Vec<String> {
    let mut files = Vec::new();
    collect_recursive(dir, &mut files);
    files.sort();
    files
}

fn collect_recursive(dir: &str, out: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_recursive(path.to_str().unwrap(), out);
        } else if path.extension().is_some_and(|e| e == "uzon") {
            out.push(path.to_string_lossy().to_string());
        }
    }
}
