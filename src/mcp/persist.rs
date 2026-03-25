use serde::{de::DeserializeOwned, Serialize};
use std::io;
use std::path::{Path, PathBuf};

/// Generic atomic JSON persistence store.
///
/// Provides load/save operations with atomic writes (write-to-tmp + rename).
/// Returns `T::default()` when the backing file does not yet exist.
pub struct JsonStore<T> {
    path: PathBuf,
    _marker: std::marker::PhantomData<T>,
}

impl<T> JsonStore<T>
where
    T: Default + Serialize + DeserializeOwned,
{
    /// Create a new store backed by the given file path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Return the backing file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load data from the backing file.
    ///
    /// Returns `T::default()` if the file does not exist.
    /// Propagates all other I/O and deserialization errors.
    pub fn load(&self) -> Result<T, JsonStoreError> {
        match std::fs::read_to_string(&self.path) {
            Ok(contents) => {
                let data = serde_json::from_str(&contents).map_err(|e| JsonStoreError::Parse {
                    path: self.path.clone(),
                    source: e,
                })?;
                Ok(data)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(T::default()),
            Err(e) => Err(JsonStoreError::Io {
                path: self.path.clone(),
                source: e,
            }),
        }
    }

    /// Save data to the backing file atomically.
    ///
    /// Writes to a `.tmp` file in the same directory, then renames
    /// (atomic on POSIX). Creates parent directories if needed.
    pub fn save(&self, data: &T) -> Result<(), JsonStoreError> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| JsonStoreError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
        }

        let json =
            serde_json::to_string_pretty(data).map_err(|e| JsonStoreError::Serialize {
                path: self.path.clone(),
                source: e,
            })?;

        let tmp_path = self.tmp_path();
        std::fs::write(&tmp_path, json.as_bytes()).map_err(|e| JsonStoreError::Io {
            path: tmp_path.clone(),
            source: e,
        })?;

        std::fs::rename(&tmp_path, &self.path).map_err(|e| JsonStoreError::Io {
            path: self.path.clone(),
            source: e,
        })?;

        Ok(())
    }

    /// Compute the temporary file path (same directory, `.tmp` suffix).
    fn tmp_path(&self) -> PathBuf {
        let mut tmp = self.path.as_os_str().to_owned();
        tmp.push(".tmp");
        PathBuf::from(tmp)
    }
}

/// Errors from `JsonStore` operations.
#[derive(Debug)]
pub enum JsonStoreError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    Serialize {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl std::fmt::Display for JsonStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "I/O error at {}: {}", path.display(), source),
            Self::Parse { path, source } => {
                write!(f, "parse error at {}: {}", path.display(), source)
            }
            Self::Serialize { path, source } => {
                write!(f, "serialize error at {}: {}", path.display(), source)
            }
        }
    }
}

impl std::error::Error for JsonStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Serialize { source, .. } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
    struct TestData {
        items: Vec<String>,
    }

    #[test]
    fn load_returns_default_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store: JsonStore<TestData> = JsonStore::new(dir.path().join("missing.json"));
        let data = store.load().unwrap();
        assert_eq!(data, TestData::default());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        let store: JsonStore<TestData> = JsonStore::new(&path);

        let data = TestData {
            items: vec!["alpha".to_string(), "beta".to_string()],
        };
        store.save(&data).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(data, loaded);
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("deep").join("data.json");
        let store: JsonStore<TestData> = JsonStore::new(&path);

        let data = TestData {
            items: vec!["test".to_string()],
        };
        store.save(&data).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(data, loaded);
    }

    #[test]
    fn save_is_atomic_no_tmp_left() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        let store: JsonStore<TestData> = JsonStore::new(&path);

        store.save(&TestData::default()).unwrap();

        let tmp_path = path.with_extension("json.tmp");
        assert!(!tmp_path.exists(), ".tmp file should not remain after save");
    }

    #[test]
    fn save_produces_pretty_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        let store: JsonStore<TestData> = JsonStore::new(&path);

        let data = TestData {
            items: vec!["a".to_string()],
        };
        store.save(&data).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains('\n'), "output should be pretty-printed");
    }

    #[test]
    fn save_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        let store: JsonStore<TestData> = JsonStore::new(&path);

        let first = TestData {
            items: vec!["first".to_string()],
        };
        store.save(&first).unwrap();

        let second = TestData {
            items: vec!["second".to_string(), "third".to_string()],
        };
        store.save(&second).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded, second);
    }

    #[test]
    fn load_returns_error_on_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not json").unwrap();

        let store: JsonStore<TestData> = JsonStore::new(&path);
        let result = store.load();
        assert!(result.is_err());
    }
}
