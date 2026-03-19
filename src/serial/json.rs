use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use std::collections::BTreeMap;

use crate::diagnostic::FatalError;
use crate::model::StatsOutput;
use super::{ClusterOutput, GraphOutput, GraphReader, GraphSerializer, RawImportOutput};

/// JSON serializer with atomic writes.
pub struct JsonSerializer;

impl GraphSerializer for JsonSerializer {
    fn write_graph(&self, output: &GraphOutput, dir: &Path) -> Result<(), FatalError> {
        ensure_dir(dir)?;
        atomic_write(dir, "graph.json", output)
    }

    fn write_clusters(&self, clusters: &ClusterOutput, dir: &Path) -> Result<(), FatalError> {
        ensure_dir(dir)?;
        atomic_write(dir, "clusters.json", clusters)
    }

    fn write_stats(&self, stats: &StatsOutput, dir: &Path) -> Result<(), FatalError> {
        ensure_dir(dir)?;
        atomic_write(dir, "stats.json", stats)
    }

    fn write_raw_imports(
        &self,
        imports: &BTreeMap<String, Vec<RawImportOutput>>,
        dir: &Path,
    ) -> Result<(), FatalError> {
        ensure_dir(dir)?;
        atomic_write(dir, "raw_imports.json", imports)
    }
}

impl GraphReader for JsonSerializer {
    fn read_graph(&self, dir: &Path) -> Result<GraphOutput, FatalError> {
        let path = dir.join("graph.json");
        let file = fs::File::open(&path).map_err(|_| FatalError::GraphNotFound {
            path: dir.to_path_buf(),
        })?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).map_err(|e| FatalError::GraphCorrupted {
            path,
            reason: e.to_string(),
        })
    }

    fn read_clusters(&self, dir: &Path) -> Result<ClusterOutput, FatalError> {
        let path = dir.join("clusters.json");
        let file = fs::File::open(&path).map_err(|_| FatalError::GraphNotFound {
            path: dir.to_path_buf(),
        })?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).map_err(|e| FatalError::GraphCorrupted {
            path,
            reason: e.to_string(),
        })
    }

    fn read_stats(&self, dir: &Path) -> Result<Option<StatsOutput>, FatalError> {
        let path = dir.join("stats.json");
        match fs::File::open(&path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                let stats: StatsOutput = serde_json::from_reader(reader).map_err(|e| {
                    FatalError::GraphCorrupted {
                        path: path.clone(),
                        reason: e.to_string(),
                    }
                })?;
                if stats.version != 1 {
                    return Err(FatalError::GraphCorrupted {
                        path,
                        reason: format!("unsupported stats version: {}", stats.version),
                    });
                }
                Ok(Some(stats))
            }
            Err(_) => Ok(None),
        }
    }

    fn read_raw_imports(
        &self,
        dir: &Path,
    ) -> Result<Option<BTreeMap<String, Vec<RawImportOutput>>>, FatalError> {
        let path = dir.join("raw_imports.json");
        match fs::File::open(&path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                let imports = serde_json::from_reader(reader).map_err(|e| {
                    FatalError::GraphCorrupted {
                        path,
                        reason: e.to_string(),
                    }
                })?;
                Ok(Some(imports))
            }
            Err(_) => Ok(None),
        }
    }
}

/// Create output directory (idempotent).
fn ensure_dir(dir: &Path) -> Result<(), FatalError> {
    fs::create_dir_all(dir).map_err(|e| FatalError::OutputNotWritable {
        path: dir.to_path_buf(),
        reason: e.to_string(),
    })
}

/// Write JSON atomically: write to a unique temp file, then rename.
fn atomic_write<T: serde::Serialize>(dir: &Path, filename: &str, value: &T) -> Result<(), FatalError> {
    let final_path = dir.join(filename);
    let tmp_path = dir.join(format!("{}.{}.tmp", filename, std::process::id()));

    let file = fs::File::create(&tmp_path).map_err(|e| FatalError::OutputNotWritable {
        path: final_path.clone(),
        reason: e.to_string(),
    })?;

    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, value).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        FatalError::OutputNotWritable {
            path: final_path.clone(),
            reason: e.to_string(),
        }
    })?;

    fs::rename(&tmp_path, &final_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        FatalError::OutputNotWritable {
            path: final_path,
            reason: e.to_string(),
        }
    })?;

    Ok(())
}
