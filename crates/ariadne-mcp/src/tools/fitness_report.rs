//! `fitness_report` shared path — the architecture-fitness verdict (block A,
//! A3).
//!
//! Reads `ariadne-fitness.toml` from the project root (ADR-0028), resolves each
//! layer's path globs against the indexed file paths into a per-file layer
//! assignment, builds the resolved [`FitnessRules`], and runs the pure
//! `ariadne_graph::fitness_check` over the catalog graph. Both the MCP
//! `fitness_report` `#[tool]` and the CLI `fitness check` command call this one
//! [`handle`], so their answers are parity by construction — mirroring the
//! `api_surface_diff` shared-handle pattern (ADR-0027).
//!
//! Cold/in-process only: the warm `DaemonQuery::FitnessReport` leg is deferred
//! to a future tier — the cold catalog suffices for the CI gate and agent
//! queries [src:
//! .claude/plans/intelligence-platform/block-a/tier-04-fitness.md step 5].

use std::collections::BTreeMap;
use std::path::Path;

use ariadne_core::{FileId, SymbolId};
use ariadne_graph::{FitnessReport, FitnessRules, Violation};
use serde::Deserialize;

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::types::{FitnessOutput, FitnessViolation};

/// Rules file read from the project root.
const RULES_FILE: &str = "ariadne-fitness.toml";

/// `ariadne-fitness.toml` schema (ADR-0028). `[[layer]]` assigns path globs to
/// a named layer; `[[rule]]` declares a forbidden dependency direction;
/// `[thresholds]` fixes the cycle / instability ceilings.
#[derive(Debug, Default, Deserialize)]
struct FitnessConfig {
    /// Named layers, each a set of path globs.
    #[serde(default)]
    layer: Vec<LayerEntry>,
    /// Forbidden dependency directions.
    #[serde(default)]
    rule: Vec<RuleEntry>,
    /// Cycle / instability ceilings.
    #[serde(default)]
    thresholds: Thresholds,
}

#[derive(Debug, Deserialize)]
struct LayerEntry {
    /// Layer name referenced by `[[rule]].forbid`.
    name: String,
    /// Unix path globs (project-root-relative) whose files belong to the layer.
    #[serde(default)]
    paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RuleEntry {
    /// The forbidden `from → to` direction.
    forbid: Forbid,
}

#[derive(Debug, Deserialize)]
struct Forbid {
    /// Depending (source) layer name.
    from: String,
    /// Depended-on (target) layer name.
    to: String,
}

#[derive(Debug, Default, Deserialize)]
struct Thresholds {
    /// Maximum tolerated dependency cycles. Defaults to `0`.
    #[serde(default)]
    max_cycles: u32,
    /// Optional per-file instability ceiling. Absent = coupling check disabled.
    #[serde(default)]
    max_instability: Option<f32>,
}

/// Run the architecture-fitness check over `cat`, reading the rules from
/// `<root>/ariadne-fitness.toml`.
///
/// # Errors
/// [`McpError::InvalidInput`] when the rules file is missing/unreadable, is
/// malformed TOML, or carries an invalid path glob.
pub fn handle(cat: &Catalog, root: &Path) -> Result<FitnessOutput, McpError> {
    let path = root.join(RULES_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| McpError::InvalidInput(format!("read {}: {e}", path.display())))?;
    let config: FitnessConfig = toml::from_str(&text)
        .map_err(|e| McpError::InvalidInput(format!("parse {}: {e}", path.display())))?;
    let rules = resolve(cat, &config)?;

    let symbol_files: BTreeMap<SymbolId, FileId> =
        cat.symbols.iter().map(|(id, m)| (*id, m.file)).collect();
    let report = cat.graph.fitness_check(&symbol_files, &rules);
    Ok(to_wire(cat, report))
}

/// Resolve the parsed config into engine-ready [`FitnessRules`]: compile each
/// layer's globs and assign every indexed file to the first layer (in
/// declaration order) one of whose globs matches its path.
///
/// # Errors
/// [`McpError::InvalidInput`] for an invalid glob pattern.
fn resolve(cat: &Catalog, config: &FitnessConfig) -> Result<FitnessRules, McpError> {
    let mut compiled: Vec<(&str, Vec<glob::Pattern>)> = Vec::with_capacity(config.layer.len());
    for layer in &config.layer {
        let mut patterns = Vec::with_capacity(layer.paths.len());
        for p in &layer.paths {
            patterns.push(glob::Pattern::new(p).map_err(|e| {
                McpError::InvalidInput(format!("layer `{}` glob `{p}`: {e}", layer.name))
            })?);
        }
        compiled.push((layer.name.as_str(), patterns));
    }

    // Iterate files in `FileId`-sorted order (BTreeMap); first matching layer in
    // declaration order wins, so resolution is deterministic.
    let mut layer_of: BTreeMap<FileId, String> = BTreeMap::new();
    for (&fid, path) in &cat.paths {
        for (name, patterns) in &compiled {
            if patterns.iter().any(|pat| pat.matches(path)) {
                layer_of.insert(fid, (*name).to_owned());
                break;
            }
        }
    }

    let forbidden = config
        .rule
        .iter()
        .map(|r| (r.forbid.from.clone(), r.forbid.to.clone()))
        .collect();

    Ok(FitnessRules {
        layer_of,
        forbidden,
        max_cycles: config.thresholds.max_cycles,
        max_instability: config.thresholds.max_instability,
    })
}

/// Project the pure graph report onto the wire output, resolving `FileId`s to
/// paths and cycle members to canonical symbol names.
fn to_wire(cat: &Catalog, report: FitnessReport) -> FitnessOutput {
    let path_of = |fid: FileId| cat.path_of(fid).unwrap_or("<unknown>").to_owned();
    let violations = report
        .violations
        .into_iter()
        .map(|v| match v {
            Violation::ForbiddenDependency {
                from_layer,
                to_layer,
                from_file,
                to_file,
            } => FitnessViolation::ForbiddenDependency {
                from_layer,
                to_layer,
                from_file: path_of(from_file),
                to_file: path_of(to_file),
            },
            Violation::Cycle { members } => FitnessViolation::Cycle {
                members: members
                    .into_iter()
                    .map(|s| {
                        cat.meta_of(s)
                            .map_or_else(|| "<unknown>".to_owned(), |m| m.name.clone())
                    })
                    .collect(),
            },
            Violation::Instability { file, instability } => FitnessViolation::Instability {
                module: path_of(file),
                instability,
            },
        })
        .collect();
    FitnessOutput {
        ok: report.ok,
        violations,
    }
}
