pub fn normalize_path(input: &str) -> String {
    input.trim().replace('\\', "/")
}
