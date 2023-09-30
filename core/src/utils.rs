pub fn strbool(value: &str) -> bool {
    ["true", "True", "t", "T", "1"].contains(&value)
}
