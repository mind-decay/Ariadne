use std::fs;
use std::io::BufWriter;
use std::path::Path;

use crate::diagnostic::FatalError;
use super::{ClusterOutput, GraphOutput, GraphSerializer};

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
