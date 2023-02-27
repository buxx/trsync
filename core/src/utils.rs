pub fn strbool(value: &str) -> bool {
    vec!["true", "True", "t", "T", "1"].contains(&value)
}
