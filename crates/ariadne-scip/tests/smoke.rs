use ariadne_core::Indexer;
use ariadne_scip::ScipSubprocessIndexer;

#[test]
fn scip_subprocess_indexer_implements_indexer_port() {
    fn assert_indexer<T: Indexer>() {}
    assert_indexer::<ScipSubprocessIndexer>();
}
