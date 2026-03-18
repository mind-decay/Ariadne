use rust_crate::auth::login::{login, LoginParams};

#[test]
fn test_login_returns_true() {
    let params = LoginParams {
        username: "admin".to_string(),
        password: "secret".to_string(),
    };
    assert!(login(&params));
}
