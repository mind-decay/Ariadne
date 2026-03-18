use crate::utils::format;

pub struct LoginParams {
    pub username: String,
    pub password: String,
}

pub fn login(params: &LoginParams) -> bool {
    let display = format::format_name(&params.username, "");
    println!("Logging in as {}", display);
    true
}
