//! Adapter-local error type. Maps every redb / postcard / io failure into
//! [`ariadne_core::StorageError`] at the trait boundary.

use thiserror::Error;

/// Storage-adapter errors. Internal to the `ariadne-storage` crate; converted
/// to [`ariadne_core::StorageError`] before crossing the port boundary.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RedbStorageError {
    /// Any redb-side failure (database, txn, table, commit, storage).
    #[error("redb backend: {0}")]
    Redb(redb::Error),
    /// postcard codec failure (de)serializing a record body.
    #[error("postcard codec: {0}")]
    Postcard(#[from] postcard::Error),
    /// Filesystem / lock / IO failure outside redb's own paths.
    #[error("storage io: {0}")]
    Io(#[from] std::io::Error),
    /// On-disk schema version differs from the binary's expected version.
    #[error("storage schema mismatch: found {found}, expected {expected}")]
    SchemaMismatch {
        /// Version read from disk.
        found: u64,
        /// Version the running binary requires.
        expected: u64,
    },
    /// On-disk record bytes failed an invariant outside postcard's reach.
    #[error("storage corrupted: {0}")]
    Corrupted(String),
    /// A registered schema-migration step failed. The write transaction is
    /// rolled back, so the database stays intact at its pre-migration version.
    #[error("storage migration {from}->{to} failed: {reason}")]
    Migration {
        /// Schema version the failed step migrated from.
        from: u64,
        /// Schema version the failed step targeted.
        to: u64,
        /// Backend failure detail reported by the step.
        reason: String,
    },
}

// Every redb sub-error converts into the umbrella `redb::Error` already, so
// we channel them through that wrapper to keep `?` ergonomic at call sites.
macro_rules! impl_from_redb {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl From<$ty> for RedbStorageError {
                fn from(err: $ty) -> Self {
                    Self::Redb(err.into())
                }
            }
        )+
    };
}

impl_from_redb!(
    redb::Error,
    redb::DatabaseError,
    redb::TransactionError,
    redb::TableError,
    redb::CommitError,
    redb::StorageError,
);

impl From<RedbStorageError> for ariadne_core::StorageError {
    fn from(err: RedbStorageError) -> Self {
        match err {
            RedbStorageError::SchemaMismatch { found, expected } => {
                Self::SchemaMismatch { found, expected }
            }
            RedbStorageError::Postcard(e) => Self::Corrupted(format!("postcard: {e}")),
            RedbStorageError::Corrupted(s) => Self::Corrupted(s),
            RedbStorageError::Migration { from, to, reason } => {
                Self::Migration { from, to, reason }
            }
            RedbStorageError::Io(e) => Self::Io(e.to_string()),
            RedbStorageError::Redb(e) => match e {
                redb::Error::Io(io) => Self::Io(io.to_string()),
                redb::Error::Corrupted(s) => Self::Corrupted(s),
                redb::Error::UpgradeRequired(v) => Self::SchemaMismatch {
                    found: u64::from(v),
                    expected: crate::adapters::redb::SCHEMA_VERSION,
                },
                other => Self::Io(other.to_string()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RedbStorageError;
    use ariadne_core::StorageError;

    /// A rolled-back migration step leaves the DB intact at its original
    /// version, so it must not surface as `Corrupted` (on-disk damage). The
    /// `From` conversion maps it to the dedicated `StorageError::Migration`.
    #[test]
    fn migration_failure_maps_to_dedicated_core_variant() {
        let err = RedbStorageError::Migration {
            from: 1,
            to: 2,
            reason: "step backend failure".to_owned(),
        };
        match StorageError::from(err) {
            StorageError::Migration { from, to, reason } => {
                assert_eq!((from, to), (1, 2));
                assert_eq!(reason, "step backend failure");
            }
            other => panic!("expected StorageError::Migration, got {other:?}"),
        }
    }
}
