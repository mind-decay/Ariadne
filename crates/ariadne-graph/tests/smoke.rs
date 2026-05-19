use ariadne_graph::GraphError;

#[test]
fn graph_error_is_constructible() {
    let e = GraphError::Other("placeholder".into());
    assert_eq!(format!("{e}"), "graph operation failed: placeholder");
}
