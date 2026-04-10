use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::diagnostic::{DiagnosticCollector, Warning, WarningCode};
use crate::model::CanonicalPath;

/// Bundler alias configuration extracted from vite.config.* or webpack.config.*.
#[derive(Clone, Debug)]
pub struct BundlerConfig {
    pub config_dir: PathBuf,
    pub aliases: BTreeMap<String, String>,
    pub modules: Vec<String>,
}

/// Parse a `vite.config.{ts,js,mjs}` file and extract `resolve.alias` entries.
pub fn parse_vite_config(
    source: &[u8],
    config_path: &Path,
    diag: &DiagnosticCollector,
) -> Option<BundlerConfig> {
    let config_dir = config_path.parent().unwrap_or(Path::new("")).to_path_buf();
    let diag_path = CanonicalPath::new(
        config_path.to_string_lossy().replace('\\', "/"),
    );

    let tree = parse_ts(source)?;
    let root = tree.root_node();

    // Find the config object: defineConfig({...}) or export default {...}
    let config_obj = find_define_config_arg(&root, source)
        .or_else(|| find_export_default_object(&root, source))?;

    let resolve_obj = find_property_object(&config_obj, source, "resolve")?;
    let alias_node = find_property_value(&resolve_obj, source, "alias")?;

    let aliases = extract_aliases(&alias_node, source, &config_dir, &diag_path, diag);

    Some(BundlerConfig {
        config_dir,
        aliases,
        modules: Vec::new(),
    })
}

/// Parse a `webpack.config.{js,ts}` file and extract `resolve.alias` and `resolve.modules`.
pub fn parse_webpack_config(
    source: &[u8],
    config_path: &Path,
    diag: &DiagnosticCollector,
) -> Option<BundlerConfig> {
    let config_dir = config_path.parent().unwrap_or(Path::new("")).to_path_buf();
    let diag_path = CanonicalPath::new(
        config_path.to_string_lossy().replace('\\', "/"),
    );

    let tree = parse_ts(source)?;
    let root = tree.root_node();

    // Find the config object: module.exports = {...} or export default {...}
    let config_obj = find_module_exports_object(&root, source)
        .or_else(|| find_export_default_object(&root, source))?;

    let resolve_obj = find_property_object(&config_obj, source, "resolve")?;

    // Extract aliases
    let aliases = if let Some(alias_node) = find_property_value(&resolve_obj, source, "alias") {
        extract_aliases(&alias_node, source, &config_dir, &diag_path, diag)
    } else {
        BTreeMap::new()
    };

    // Extract resolve.modules
    let modules = if let Some(modules_node) = find_property_value(&resolve_obj, source, "modules") {
        extract_string_array(&modules_node, source)
    } else {
        Vec::new()
    };

    if aliases.is_empty() && modules.is_empty() {
        return None;
    }

    Some(BundlerConfig {
        config_dir,
        aliases,
        modules,
    })
}

// ---------------------------------------------------------------------------
// Tree-sitter helpers
// ---------------------------------------------------------------------------

fn parse_ts(source: &[u8]) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter::Language::from(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        ))
        .ok()?;
    parser.parse(source, None)
}

/// Find `defineConfig({...})` call and return the object argument.
fn find_define_config_arg<'a>(
    root: &tree_sitter::Node<'a>,
    source: &'a [u8],
) -> Option<tree_sitter::Node<'a>> {
    find_node_recursive(root, source, |node, src| {
        if node.kind() != "call_expression" {
            return None;
        }
        let func = node.child_by_field_name("function")?;
        let func_text = func.utf8_text(src).ok()?;
        if func_text != "defineConfig" {
            return None;
        }
        let args = node.child_by_field_name("arguments")?;
        for i in 0..args.child_count() {
            if let Some(arg) = args.child(i) {
                // Direct object: defineConfig({ ... })
                if arg.kind() == "object" {
                    return Some(arg);
                }
                // Callback: defineConfig(({ mode }) => { return { ... } })
                // or defineConfig(function({ mode }) { return { ... } })
                if arg.kind() == "arrow_function" || arg.kind() == "function" {
                    return find_returned_object(&arg, src);
                }
            }
        }
        None
    })
}

/// Find a `return { ... }` statement inside a function body and return the object.
fn find_returned_object<'a>(
    func_node: &tree_sitter::Node<'a>,
    source: &'a [u8],
) -> Option<tree_sitter::Node<'a>> {
    find_node_recursive(func_node, source, |node, _src| {
        if node.kind() != "return_statement" {
            return None;
        }
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "object" {
                    return Some(child);
                }
                // return { ... } as Type / satisfies Type
                if child.kind() == "as_expression" || child.kind() == "satisfies_expression" {
                    for j in 0..child.child_count() {
                        if let Some(inner) = child.child(j) {
                            if inner.kind() == "object" {
                                return Some(inner);
                            }
                        }
                    }
                }
            }
        }
        None
    })
}

/// Find `export default {...}` and return the object node.
fn find_export_default_object<'a>(
    root: &'a tree_sitter::Node<'a>,
    source: &[u8],
) -> Option<tree_sitter::Node<'a>> {
    for i in 0..root.child_count() {
        let child = root.child(i)?;
        if child.kind() == "export_statement" {
            // Look for "default" keyword and then an object or satisfies_expression
            let mut has_default = false;
            for j in 0..child.child_count() {
                if let Some(gc) = child.child(j) {
                    if gc.utf8_text(source).ok() == Some("default") {
                        has_default = true;
                    }
                    if has_default && gc.kind() == "object" {
                        return Some(gc);
                    }
                    // export default { ... } satisfies Config
                    if has_default && gc.kind() == "satisfies_expression" {
                        for k in 0..gc.child_count() {
                            if let Some(inner) = gc.child(k) {
                                if inner.kind() == "object" {
                                    return Some(inner);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Find `module.exports = {...}` and return the object node.
fn find_module_exports_object<'a>(
    root: &tree_sitter::Node<'a>,
    source: &'a [u8],
) -> Option<tree_sitter::Node<'a>> {
    find_node_recursive(root, source, |node, src| {
        if node.kind() != "expression_statement" {
            return None;
        }
        let expr = node.child(0)?;
        if expr.kind() != "assignment_expression" {
            return None;
        }
        let left = expr.child_by_field_name("left")?;
        let left_text = left.utf8_text(src).ok()?;
        if left_text != "module.exports" {
            return None;
        }
        let right = expr.child_by_field_name("right")?;
        if right.kind() == "object" {
            return Some(right);
        }
        None
    })
}

/// Find a property with the given name in an object node and return the value as an object.
fn find_property_object<'a>(
    obj: &'a tree_sitter::Node<'a>,
    source: &[u8],
    name: &str,
) -> Option<tree_sitter::Node<'a>> {
    let value = find_property_value(obj, source, name)?;
    if value.kind() == "object" {
        Some(value)
    } else {
        None
    }
}

/// Find a property with the given name in an object node and return the value node.
fn find_property_value<'a>(
    obj: &'a tree_sitter::Node<'a>,
    source: &[u8],
    name: &str,
) -> Option<tree_sitter::Node<'a>> {
    for i in 0..obj.child_count() {
        let child = obj.child(i)?;
        if child.kind() == "pair" || child.kind() == "property_assignment" {
            let key = child.child_by_field_name("key")?;
            let key_text = key.utf8_text(source).ok()?;
            // Key may be a string literal with quotes or a bare identifier
            let key_clean = key_text.trim_matches(|c| c == '\'' || c == '"');
            if key_clean == name {
                return child.child_by_field_name("value");
            }
        }
        // Shorthand property: { resolve } — skip, not useful here
    }
    None
}

/// Extract alias entries from an alias value node.
/// Handles both object form `{ '@': './src' }` and array form `[{ find, replacement }]`.
fn extract_aliases(
    node: &tree_sitter::Node,
    source: &[u8],
    config_dir: &Path,
    diag_path: &CanonicalPath,
    diag: &DiagnosticCollector,
) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();

    match node.kind() {
        "object" => {
            // Object form: { '@': './src', '~': fileURLToPath(...) }
            for i in 0..node.child_count() {
                if let Some(pair) = node.child(i) {
                    if pair.kind() != "pair" && pair.kind() != "property_assignment" {
                        continue;
                    }
                    let key = match pair.child_by_field_name("key") {
                        Some(k) => k,
                        None => continue,
                    };
                    let key_text = match key.utf8_text(source).ok() {
                        Some(t) => strip_quotes(t).to_string(),
                        None => continue,
                    };
                    let value = match pair.child_by_field_name("value") {
                        Some(v) => v,
                        None => continue,
                    };

                    if let Some(resolved) = resolve_alias_value(&value, source, config_dir) {
                        aliases.insert(key_text, resolved);
                    } else {
                        let value_text = value.utf8_text(source).unwrap_or("<unknown>");
                        diag.warn(Warning {
                            code: WarningCode::W045DynamicAliasSkipped,
                            path: diag_path.clone(),
                            message: format!(
                                "unrecognized alias value for '{}', skipping",
                                key_text
                            ),
                            detail: Some(value_text.to_string()),
                        });
                    }
                }
            }
        }
        "array" => {
            // Array form: [{ find: '@', replacement: './src' }]
            for i in 0..node.child_count() {
                if let Some(elem) = node.child(i) {
                    if elem.kind() != "object" {
                        continue;
                    }
                    let find_val = find_property_value(&elem, source, "find");
                    let repl_val = find_property_value(&elem, source, "replacement");

                    if let (Some(f), Some(r)) = (find_val, repl_val) {
                        let find_text = match extract_string_literal(&f, source) {
                            Some(s) => s,
                            None => continue,
                        };
                        if let Some(resolved) = resolve_alias_value(&r, source, config_dir) {
                            aliases.insert(find_text, resolved);
                        } else {
                            let value_text = r.utf8_text(source).unwrap_or("<unknown>");
                            diag.warn(Warning {
                                code: WarningCode::W045DynamicAliasSkipped,
                                path: diag_path.clone(),
                                message: format!(
                                    "unrecognized alias value for '{}', skipping",
                                    find_text
                                ),
                                detail: Some(value_text.to_string()),
                            });
                        }
                    }
                }
            }
        }
        _ => {}
    }

    aliases
}

/// Resolve an alias value node to a path string relative to config_dir.
fn resolve_alias_value(
    node: &tree_sitter::Node,
    source: &[u8],
    config_dir: &Path,
) -> Option<String> {
    // Pattern 1: String literal './src' or 'src'
    if let Some(s) = extract_string_literal(node, source) {
        return Some(normalize_alias_path(&s, config_dir));
    }

    // Pattern 2: fileURLToPath(new URL('./src', import.meta.url))
    if node.kind() == "call_expression" {
        let func = node.child_by_field_name("function")?;
        let func_text = func.utf8_text(source).ok()?;
        if func_text == "fileURLToPath" {
            let args = node.child_by_field_name("arguments")?;
            // First arg should be `new URL('./src', import.meta.url)`
            for i in 0..args.child_count() {
                if let Some(arg) = args.child(i) {
                    if arg.kind() == "new_expression" {
                        let new_args = arg.child_by_field_name("arguments")?;
                        // First argument of URL constructor is the relative path
                        for j in 0..new_args.child_count() {
                            if let Some(url_arg) = new_args.child(j) {
                                if let Some(s) = extract_string_literal(&url_arg, source) {
                                    return Some(normalize_alias_path(&s, config_dir));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Pattern 3: path.resolve(__dirname, 'src') or path.join(__dirname, 'src')
    if node.kind() == "call_expression" {
        let func = node.child_by_field_name("function")?;
        let func_text = func.utf8_text(source).ok()?;
        if func_text == "path.resolve" || func_text == "path.join" {
            let args = node.child_by_field_name("arguments")?;
            // Expect: __dirname, 'literal_path'
            let mut found_dirname = false;
            for i in 0..args.child_count() {
                if let Some(arg) = args.child(i) {
                    let arg_text = arg.utf8_text(source).ok().unwrap_or("");
                    if arg_text == "__dirname" {
                        found_dirname = true;
                        continue;
                    }
                    if found_dirname {
                        if let Some(s) = extract_string_literal(&arg, source) {
                            return Some(normalize_alias_path(&s, config_dir));
                        }
                    }
                }
            }
        }
    }

    // Pattern 4: __dirname + '/src' (binary expression)
    if node.kind() == "binary_expression" {
        let left = node.child_by_field_name("left")?;
        let left_text = left.utf8_text(source).ok()?;
        if left_text == "__dirname" {
            let right = node.child_by_field_name("right")?;
            if let Some(s) = extract_string_literal(&right, source) {
                let clean = s.trim_start_matches('/');
                return Some(normalize_alias_path(clean, config_dir));
            }
        }
    }

    // Pattern 5: Template literal with single prefix variable `${dirname}/src/app`
    // The variable is a dir reference; extract the static path suffix.
    if node.kind() == "template_string" {
        if let Some(path_suffix) = extract_template_path_suffix(node, source) {
            return Some(normalize_alias_path(&path_suffix, config_dir));
        }
    }

    None
}

/// Extract the static path suffix from a template literal like `${dirname}/src/app`.
///
/// Recognizes: exactly one `template_substitution` at the start, followed by a
/// `string_fragment` containing `/path`. Returns `path` (stripped leading `/`).
fn extract_template_path_suffix(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut found_substitution = false;
    let mut suffix: Option<String> = None;

    for i in 0..node.child_count() {
        let child = match node.child(i) {
            Some(c) => c,
            None => continue,
        };
        match child.kind() {
            "`" => continue,
            "template_substitution" => {
                if found_substitution {
                    return None; // Multiple interpolations — too complex
                }
                found_substitution = true;
            }
            "string_fragment" if found_substitution => {
                let text = child.utf8_text(source).ok()?;
                let clean = text.trim_start_matches('/');
                if clean.is_empty() {
                    continue;
                }
                suffix = Some(clean.to_string());
            }
            _ => {}
        }
    }

    if found_substitution {
        suffix
    } else {
        None
    }
}

/// Extract a string literal value (removing quotes).
fn extract_string_literal(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    if node.kind() == "string" {
        let text = node.utf8_text(source).ok()?;
        Some(strip_quotes(text).to_string())
    } else if node.kind() == "template_string" {
        // Only extract simple template strings without interpolation.
        // `\`./src\`` is fine, but `\`${dirname}/src\`` has template_substitution children.
        let has_interpolation = (0..node.child_count()).any(|i| {
            node.child(i)
                .is_some_and(|c| c.kind() == "template_substitution")
        });
        if has_interpolation {
            return None; // → W045 will be emitted by caller
        }
        let text = node.utf8_text(source).ok()?;
        Some(strip_quotes(text).to_string())
    } else {
        None
    }
}

/// Extract an array of string literals from an array node.
fn extract_string_array(node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
    let mut result = Vec::new();
    if node.kind() != "array" {
        return result;
    }
    for i in 0..node.child_count() {
        if let Some(elem) = node.child(i) {
            if let Some(s) = extract_string_literal(&elem, source) {
                result.push(s);
            }
        }
    }
    result
}

/// Normalize an alias path: strip leading ./, join with config_dir if non-empty.
fn normalize_alias_path(path: &str, config_dir: &Path) -> String {
    let cleaned = path.strip_prefix("./").unwrap_or(path);
    if config_dir == Path::new("") || config_dir == Path::new(".") {
        cleaned.to_string()
    } else {
        let joined = config_dir.join(cleaned);
        joined.to_string_lossy().replace('\\', "/")
    }
}

fn strip_quotes(s: &str) -> &str {
    s.trim_matches(|c| c == '\'' || c == '"' || c == '`')
}

/// Recursively search for an expression_statement or export_statement matching a predicate.
/// Returns the mapped result from the first match. Uses iterative depth-first search
/// to avoid lifetime issues with tree-sitter's `Node::child()` returning owned nodes.
fn find_node_recursive<'a, F>(
    root: &tree_sitter::Node<'a>,
    source: &'a [u8],
    f: F,
) -> Option<tree_sitter::Node<'a>>
where
    F: Fn(&tree_sitter::Node<'a>, &'a [u8]) -> Option<tree_sitter::Node<'a>>,
{
    let mut cursor = root.walk();
    let mut visited = false;
    loop {
        if !visited {
            let node = cursor.node();
            if let Some(result) = f(&node, source) {
                return Some(result);
            }
        }
        if !visited && cursor.goto_first_child() {
            visited = false;
            continue;
        }
        if cursor.goto_next_sibling() {
            visited = false;
            continue;
        }
        if !cursor.goto_parent() {
            break;
        }
        visited = true;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vite_config(source: &str) -> Option<BundlerConfig> {
        let diag = DiagnosticCollector::new();
        parse_vite_config(source.as_bytes(), Path::new("vite.config.ts"), &diag)
    }

    fn vite_config_in_dir(source: &str, dir: &str) -> Option<BundlerConfig> {
        let diag = DiagnosticCollector::new();
        let path = PathBuf::from(dir).join("vite.config.ts");
        parse_vite_config(source.as_bytes(), &path, &diag)
    }

    fn webpack_config(source: &str) -> Option<BundlerConfig> {
        let diag = DiagnosticCollector::new();
        parse_webpack_config(source.as_bytes(), Path::new("webpack.config.js"), &diag)
    }

    // --- Vite: string literal alias ---

    #[test]
    fn vite_string_literal_alias() {
        let source = r#"
import { defineConfig } from 'vite';
export default defineConfig({
  resolve: {
    alias: {
      '@': './src',
      '~': './lib',
    }
  }
});
"#;
        let config = vite_config(source).unwrap();
        assert_eq!(config.aliases.get("@").unwrap(), "src");
        assert_eq!(config.aliases.get("~").unwrap(), "lib");
        assert!(config.modules.is_empty());
    }

    // --- Vite: fileURLToPath idiom ---

    #[test]
    fn vite_file_url_to_path_alias() {
        let source = r#"
import { defineConfig } from 'vite';
import { fileURLToPath } from 'url';
export default defineConfig({
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    }
  }
});
"#;
        let config = vite_config(source).unwrap();
        assert_eq!(config.aliases.get("@").unwrap(), "src");
    }

    // --- Vite: array form alias ---

    #[test]
    fn vite_array_form_alias() {
        let source = r#"
import { defineConfig } from 'vite';
export default defineConfig({
  resolve: {
    alias: [
      { find: '@', replacement: './src' },
      { find: '~utils', replacement: './src/utils' },
    ]
  }
});
"#;
        let config = vite_config(source).unwrap();
        assert_eq!(config.aliases.get("@").unwrap(), "src");
        assert_eq!(config.aliases.get("~utils").unwrap(), "src/utils");
    }

    // --- Vite: export default without defineConfig ---

    #[test]
    fn vite_export_default_no_define_config() {
        let source = r#"
export default {
  resolve: {
    alias: {
      '@': './src',
    }
  }
};
"#;
        let config = vite_config(source).unwrap();
        assert_eq!(config.aliases.get("@").unwrap(), "src");
    }

    // --- Vite: W045 for unrecognized alias ---

    #[test]
    fn vite_dynamic_alias_emits_w045() {
        let source = r#"
import { defineConfig } from 'vite';
export default defineConfig({
  resolve: {
    alias: {
      '@': getAliases(),
    }
  }
});
"#;
        let diag = DiagnosticCollector::new();
        let config = parse_vite_config(
            source.as_bytes(),
            Path::new("vite.config.ts"),
            &diag,
        );
        // Config may still parse (with empty aliases)
        let report = diag.drain();
        assert!(
            report.warnings.iter().any(|w| w.code == WarningCode::W045DynamicAliasSkipped),
            "expected W045 for dynamic alias"
        );
        if let Some(c) = config {
            assert!(!c.aliases.contains_key("@"));
        }
    }

    // --- Vite: config in subdirectory ---

    #[test]
    fn vite_config_in_subdirectory() {
        let source = r#"
import { defineConfig } from 'vite';
export default defineConfig({
  resolve: {
    alias: {
      '@': './src',
    }
  }
});
"#;
        let config = vite_config_in_dir(source, "packages/app").unwrap();
        assert_eq!(config.config_dir, PathBuf::from("packages/app"));
        assert_eq!(config.aliases.get("@").unwrap(), "packages/app/src");
    }

    // --- Vite: defineConfig(callback) ---

    #[test]
    fn vite_define_config_callback() {
        let source = r#"
import { defineConfig } from 'vite';
export default defineConfig(({ mode }) => {
  return {
    resolve: {
      alias: {
        '@': './src',
        'shared': './src/shared',
      }
    }
  };
});
"#;
        let config = vite_config(source).unwrap();
        assert_eq!(config.aliases.get("@").unwrap(), "src");
        assert_eq!(config.aliases.get("shared").unwrap(), "src/shared");
    }

    #[test]
    fn vite_define_config_callback_with_type_annotation() {
        let source = r#"
import { defineConfig, UserConfig } from 'vite';
export default defineConfig(({ mode }): UserConfig => {
  return {
    resolve: {
      alias: {
        app: './src/app',
      }
    }
  };
});
"#;
        let config = vite_config(source).unwrap();
        assert_eq!(config.aliases.get("app").unwrap(), "src/app");
    }

    // --- Vite: template literal with variable prefix ---

    #[test]
    fn vite_template_literal_alias() {
        let source = r#"
import { defineConfig } from 'vite';
const dirname = '/project';
export default defineConfig({
  resolve: {
    alias: {
      app: `${dirname}/src/app`,
      shared: `${dirname}/src/shared`,
    }
  }
});
"#;
        let config = vite_config(source).unwrap();
        assert_eq!(config.aliases.get("app").unwrap(), "src/app");
        assert_eq!(config.aliases.get("shared").unwrap(), "src/shared");
    }

    // --- Vite: no resolve section ---

    #[test]
    fn vite_no_resolve_returns_none() {
        let source = r#"
import { defineConfig } from 'vite';
export default defineConfig({
  server: { port: 3000 }
});
"#;
        assert!(vite_config(source).is_none());
    }

    // --- Webpack: path.resolve alias ---

    #[test]
    fn webpack_path_resolve_alias() {
        let source = r#"
const path = require('path');
module.exports = {
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'),
    }
  }
};
"#;
        let config = webpack_config(source).unwrap();
        assert_eq!(config.aliases.get("@").unwrap(), "src");
    }

    // --- Webpack: path.join alias ---

    #[test]
    fn webpack_path_join_alias() {
        let source = r#"
const path = require('path');
module.exports = {
  resolve: {
    alias: {
      '@': path.join(__dirname, 'src'),
    }
  }
};
"#;
        let config = webpack_config(source).unwrap();
        assert_eq!(config.aliases.get("@").unwrap(), "src");
    }

    // --- Webpack: __dirname + '/src' ---

    #[test]
    fn webpack_dirname_concat_alias() {
        let source = r#"
module.exports = {
  resolve: {
    alias: {
      '@': __dirname + '/src',
    }
  }
};
"#;
        let config = webpack_config(source).unwrap();
        assert_eq!(config.aliases.get("@").unwrap(), "src");
    }

    // --- Webpack: export default (ESM) ---

    #[test]
    fn webpack_export_default_alias() {
        let source = r#"
export default {
  resolve: {
    alias: {
      '@': './src',
    }
  }
};
"#;
        let config = webpack_config(source).unwrap();
        assert_eq!(config.aliases.get("@").unwrap(), "src");
    }

    // --- Webpack: resolve.modules ---

    #[test]
    fn webpack_resolve_modules() {
        let source = r#"
module.exports = {
  resolve: {
    modules: ['node_modules', 'src'],
    alias: {
      '@': './src',
    }
  }
};
"#;
        let config = webpack_config(source).unwrap();
        assert_eq!(config.modules, vec!["node_modules", "src"]);
        assert_eq!(config.aliases.get("@").unwrap(), "src");
    }

    // --- Webpack: W045 for dynamic alias ---

    #[test]
    fn webpack_dynamic_alias_emits_w045() {
        let source = r#"
module.exports = {
  resolve: {
    alias: {
      '@': getAliases(),
    }
  }
};
"#;
        let diag = DiagnosticCollector::new();
        parse_webpack_config(
            source.as_bytes(),
            Path::new("webpack.config.js"),
            &diag,
        );
        let report = diag.drain();
        assert!(
            report.warnings.iter().any(|w| w.code == WarningCode::W045DynamicAliasSkipped),
            "expected W045 for dynamic alias"
        );
    }

    // --- Webpack: no alias returns None ---

    #[test]
    fn webpack_no_alias_returns_none() {
        let source = r#"
module.exports = {
  entry: './src/index.js',
};
"#;
        assert!(webpack_config(source).is_none());
    }

    // --- Webpack: string literal alias ---

    #[test]
    fn webpack_string_literal_alias() {
        let source = r#"
module.exports = {
  resolve: {
    alias: {
      '@': './src',
      'components': './src/components',
    }
  }
};
"#;
        let config = webpack_config(source).unwrap();
        assert_eq!(config.aliases.get("@").unwrap(), "src");
        assert_eq!(config.aliases.get("components").unwrap(), "src/components");
    }
}
