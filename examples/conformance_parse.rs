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

    // Collect invalid .uzon files recursively
    let invalid_files = collect_uzon_files(&format!("{}/invalid", base));
    for path in &invalid_files {
        let source = std::fs::read_to_string(path).unwrap();
        match uzon::from_str(&source) {
            Err(_) => pass += 1,
            Ok(_) => {
                fail += 1;
                let name = path.strip_prefix(base).unwrap_or(path);
                failures.push(format!("  FAIL (should reject) {}", name));
            }
        }
    }

    println!(
        "=== Parse conformance: {}/{} passed (valid: {}, invalid: {}) ===",
        pass,
        pass + fail,
        valid_files.len(),
        invalid_files.len()
    );
    for f in &failures {
        println!("{}", f);
    }
    if fail > 0 {
        std::process::exit(1);
    }
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
