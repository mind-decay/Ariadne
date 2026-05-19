use ariadne_mcp::McpError;

#[test]
fn mcp_error_is_constructible() {
    let e = McpError::Other("placeholder".into());
    assert_eq!(format!("{e}"), "mcp: placeholder");
}
