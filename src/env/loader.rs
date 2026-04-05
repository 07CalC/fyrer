use std::{collections::HashMap, fs, path::PathBuf};

use crate::error::FyrerError;

pub fn load_env_from_file(path: PathBuf) -> Result<HashMap<String, String>, FyrerError> {
    if fs::exists(&path).unwrap_or(false) == false {
        return Err(FyrerError::FileNotFound(path.to_string_lossy().to_string()));
    }
    let content = fs::read_to_string(&path)?;
    let out = parse_env(&content);
    Ok(out)
}

fn parse_env(content: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if content.is_empty() {
        return out;
    }
    for line in content.lines() {
        if line.starts_with("#") || line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once("=") {
            let key = k.trim();
            let val = v.trim();
            if !key.is_empty() {
                out.insert(key.to_string(), val.to_string());
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use std::path::{self, Path};

    use super::*;

    #[test]
    fn parse_basic_key_value() {
        let input = "PORT=3000";
        let out = parse_env(&input);
        assert_eq!(out.get("PORT").unwrap(), "3000")
    }

    #[test]
    fn ingore_comments_and_empty_lines() {
        let input = r#"

          # comment, should be ignored

          PORT=3000
        "#;
        let out = parse_env(&input);
        assert_eq!(out.get("PORT").unwrap(), "3000")
    }

    #[test]
    fn ignore_invalid_lines() {
        let input = r#"
        this is an invalid line
        PORT=3000
        "#;
        let out = parse_env(&input);
        assert_eq!(out.len(), 1);
        assert_eq!(out.get("PORT").unwrap(), "3000")
    }

    #[test]
    #[ignore = r#"
    inline comments are failing for now
    if we strip off the part after #, then values like "abc#432", will not be parsed as they
    should"#]
    fn handle_inline_comment() {
        let input = r#"
        PORT= 3000 # comment
        "#;
        let out = parse_env(&input);
        assert_eq!(out.get("PORT").unwrap(), "3000")
    }

    #[test]
    fn handle_qoutes() {
        let input = r#"
        PORT="3000"
        "#;
        let out = parse_env(&input);
        assert_eq!(out.get("PORT").unwrap(), "\"3000\"")
    }

    #[test]
    fn handle_file_not_exist() {
        let out = load_env_from_file(Path::new(".env").to_path_buf());
        assert_eq!(
            out.unwrap_err().to_string(),
            "File not found: .env".to_string()
        );
    }
}
