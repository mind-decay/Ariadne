use ariadne_e2e::E2eError;

#[test]
fn e2e_error_is_constructible() {
    let e = E2eError::Other("placeholder".into());
    assert_eq!(format!("{e}"), "e2e operation failed: placeholder");
}
