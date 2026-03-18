pub fn format_date(date: &str) -> String {
    format!("Date: {}", date)
}

pub fn format_name(first: &str, last: &str) -> String {
    format!("{} {}", first, last).trim().to_string()
}
