//! Turbopack / Turborepo pipeline configuration parsing.
//!
//! Parses `turbo.json` to extract pipeline task dependencies and outputs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::diagnostic::{DiagnosticCollector, Warning, WarningCode};
use crate::model::CanonicalPath;
use crate::parser::config::jsonc;

/// Turbo pipeline configuration.
#[derive(Clone, Debug)]
pub struct TurboConfig {
    pub config_dir: PathBuf,
    pub pipeline: BTreeMap<String, TurboPipelineEntry>,
}

/// A single pipeline task entry.
#[derive(Clone, Debug)]
pub struct TurboPipelineEntry {
    pub depends_on: Vec<String>,
    pub outputs: Vec<String>,
}

/// Parse a `turbo.json` file and extract pipeline configuration.
pub fn parse_turbo_config(
    source: &[u8],
    config_path: &Path,
    diag: &DiagnosticCollector,
) -> Option<TurboConfig> {
    let config_dir = config_path.parent().unwrap_or(Path::new("")).to_path_buf();
    let diag_path = CanonicalPath::new(
        config_path.to_string_lossy().replace('\\', "/"),
    );

    let source_str = std::str::from_utf8(source).ok()?;
    let clean = jsonc::strip_jsonc_comments(source_str);
    let value: serde_json::Value = match serde_json::from_str(&clean) {
        Ok(v) => v,
        Err(e) => {
            diag.warn(Warning {
                code: WarningCode::W047TurboConfigParseError,
                path: diag_path,
                message: "failed to parse turbo.json".to_string(),
                detail: Some(e.to_string()),
            });
            return None;
        }
    };

    let obj = value.as_object()?;

    // Turbo v2 uses "tasks", v1 uses "pipeline"
    let pipeline_obj = obj
        .get("tasks")
        .and_then(|v| v.as_object())
        .or_else(|| obj.get("pipeline").and_then(|v| v.as_object()))?;

    let mut pipeline = BTreeMap::new();
    for (task_name, task_value) in pipeline_obj {
        let task_obj = match task_value.as_object() {
            Some(o) => o,
            None => continue,
        };

        let mut depends_on: Vec<String> = task_obj
            .get("dependsOn")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        depends_on.sort(); // D-006: deterministic output

        let mut outputs: Vec<String> = task_obj
            .get("outputs")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        outputs.sort(); // D-006: deterministic output

        pipeline.insert(
            task_name.clone(),
            TurboPipelineEntry {
                depends_on,
                outputs,
            },
        );
    }

    if pipeline.is_empty() {
        return None;
    }

    Some(TurboConfig {
        config_dir,
        pipeline,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Option<TurboConfig> {
        let diag = DiagnosticCollector::new();
        parse_turbo_config(source.as_bytes(), Path::new("turbo.json"), &diag)
    }

    #[test]
    fn turbo_v1_pipeline() {
        let source = r#"{
  "pipeline": {
    "build": {
      "dependsOn": ["^build"],
      "outputs": ["dist/**", ".next/**"]
    },
    "test": {
      "dependsOn": ["build"]
    },
    "lint": {}
  }
}"#;
        let config = parse(source).unwrap();
        assert_eq!(config.pipeline.len(), 3);

        let build = &config.pipeline["build"];
        assert_eq!(build.depends_on, vec!["^build"]);
        assert_eq!(build.outputs, vec![".next/**", "dist/**"]); // sorted

        let test = &config.pipeline["test"];
        assert_eq!(test.depends_on, vec!["build"]);
        assert!(test.outputs.is_empty());

        let lint = &config.pipeline["lint"];
        assert!(lint.depends_on.is_empty());
        assert!(lint.outputs.is_empty());
    }

    #[test]
    fn turbo_v2_tasks() {
        let source = r#"{
  "tasks": {
    "build": {
      "dependsOn": ["^build"],
      "outputs": [".next/**"]
    }
  }
}"#;
        let config = parse(source).unwrap();
        assert_eq!(config.pipeline.len(), 1);
        assert!(config.pipeline.contains_key("build"));
    }

    #[test]
    fn turbo_empty_pipeline_returns_none() {
        let source = r#"{ "pipeline": {} }"#;
        assert!(parse(source).is_none());
    }

    #[test]
    fn turbo_invalid_json_emits_w047() {
        let diag = DiagnosticCollector::new();
        let result = parse_turbo_config(
            b"{ invalid json }",
            Path::new("turbo.json"),
            &diag,
        );
        assert!(result.is_none());
        let report = diag.drain();
        assert!(report
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::W047TurboConfigParseError));
    }

    #[test]
    fn turbo_depends_on_sorted() {
        let source = r#"{
  "pipeline": {
    "deploy": {
      "dependsOn": ["test", "lint", "build"]
    }
  }
}"#;
        let config = parse(source).unwrap();
        assert_eq!(
            config.pipeline["deploy"].depends_on,
            vec!["build", "lint", "test"]
        );
    }
}
