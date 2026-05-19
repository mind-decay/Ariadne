use ariadne_salsa::SalsaError;

#[test]
fn salsa_error_is_constructible() {
    let e = SalsaError::Other("placeholder".into());
    assert_eq!(format!("{e}"), "salsa operation failed: placeholder");
}
