use ariadne_core::Storage;
use ariadne_storage::RedbStorage;

#[test]
fn redb_storage_implements_storage_port() {
    fn assert_storage<T: Storage>() {}
    assert_storage::<RedbStorage>();
}
