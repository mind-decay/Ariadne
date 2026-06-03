use ariadne_mcp::McpError;

#[test]
fn mcp_error_is_constructible() {
    let e = McpError::Other("placeholder".into());
    assert_eq!(format!("{e}"), "mcp: placeholder");
}

#[test]
fn invalid_input_maps_to_invalid_params_others_to_internal_error() {
    // Audit I3: caller-supplied bad arguments (invalid regex / path glob) must
    // surface as JSON-RPC `invalid_params` so a client can distinguish them
    // from a server fault; every other variant stays `internal_error`.
    let bad_input = McpError::InvalidInput("regex `(`: unterminated".into()).into_rmcp();
    assert_eq!(bad_input.code, rmcp::model::ErrorCode::INVALID_PARAMS);

    let server_fault = McpError::Other("placeholder".into()).into_rmcp();
    assert_eq!(server_fault.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
    let not_found = McpError::NotFound("symbol x".into()).into_rmcp();
    assert_eq!(not_found.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
}
