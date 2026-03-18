use std::path::Path;
use tempfile::TempDir;

/// Generate a synthetic project with the given parameters.
/// Creates valid source files with import statements pointing to other generated files.
pub fn generate_synthetic_project(
    file_count: usize,
    dir_count: usize,
    imports_per_file: usize,
    language: &str,
) -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create directory structure
    let dirs: Vec<String> = (0..dir_count).map(|i| format!("dir_{}", i)).collect();

    for dir in &dirs {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }

    let ext = match language {
        "typescript" => "ts",
        "go" => "go",
        "python" => "py",
        _ => "ts",
    };

    // Create files with imports
    let mut files: Vec<String> = Vec::new();
    for i in 0..file_count {
        let dir_idx = i % dir_count.max(1);
        let dir = if dir_count > 0 { &dirs[dir_idx] } else { "." };
        let filename = format!("{}/file_{}.{}", dir, i, ext);
        files.push(filename);
    }

    for (i, file_path) in files.iter().enumerate() {
        let mut content = String::new();

        // Add imports to other files
        for j in 0..imports_per_file.min(file_count - 1) {
            let target_idx = (i + j + 1) % file_count;
            let target = &files[target_idx];
            let target_rel = make_relative(file_path, target);

            match language {
                "typescript" => {
                    content.push_str(&format!(
                        "import {{ item_{} }} from '{}';\n",
                        target_idx,
                        strip_ext(&target_rel)
                    ));
                }
                "go" => {
                    if i == 0 && j == 0 {
                        content.push_str("package main\n\n");
                    }
                    // Go imports are package-level, harder to synthesize realistically
                    // Just create valid Go files
                }
                "python" => {
                    let module = strip_ext(&target_rel).replace('/', ".");
                    content.push_str(&format!("from {} import item_{}\n", module, target_idx));
                }
                _ => {}
            }
        }

        // Add an export
        match language {
            "typescript" => {
                content.push_str(&format!("export const item_{} = {};\n", i, i));
            }
            "go" => {
                if !content.contains("package") {
                    content.push_str("package main\n\n");
                }
                content.push_str(&format!("var Item{} = {}\n", i, i));
            }
            "python" => {
                content.push_str(&format!("item_{} = {}\n", i, i));
            }
            _ => {}
        }

        let full_path = root.join(file_path);
        std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        std::fs::write(full_path, content).unwrap();
    }

    tmp
}

fn make_relative(from: &str, to: &str) -> String {
    // Simple relative path calculation
    let from_dir = Path::new(from).parent().unwrap_or(Path::new(""));
    let to_path = Path::new(to);

    if from_dir == to_path.parent().unwrap_or(Path::new("")) {
        format!("./{}", to_path.file_name().unwrap().to_str().unwrap())
    } else {
        format!("../{}", to)
    }
}

fn strip_ext(path: &str) -> String {
    Path::new(path)
        .with_extension("")
        .to_string_lossy()
        .to_string()
}
