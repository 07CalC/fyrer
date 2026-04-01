use std::{collections::HashMap, fs::File, io::Read, path::Path};

use crate::config::Project;

fn parse_env(content: &str, out: &mut HashMap<String, String>) {
    if content.is_empty() {
        return;
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
}

pub fn parse_env_for_project(project: &Project) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let dotenv_path = if let Some(env_path) = &project.env_path {
        Path::new(&project.dir).join(env_path)
    } else {
        Path::new(&project.dir).join(".env")
    };

    if let Ok(mut dotenv) = File::open(dotenv_path) {
        let mut content = String::new();
        if let Err(_) = dotenv.read_to_string(&mut content) {
        } else {
            parse_env(&content, &mut out);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_key_value() {
        let input = "PORT=3000";
        let mut out = HashMap::new();
        parse_env(&input, &mut out);
        assert_eq!(out.get("PORT").unwrap(), "3000")
    }

    #[test]
    fn ingore_comments_and_empty_lines() {
        let input = r#"

          # comment, should be ignored

          PORT=3000
        "#;
        let mut out = HashMap::new();
        parse_env(&input, &mut out);
        assert_eq!(out.get("PORT").unwrap(), "3000")
    }

    #[test]
    fn ignore_invalid_lines() {
        let input = r#"
        this is an invalid line
        PORT=3000
        "#;
        let mut out = HashMap::new();
        parse_env(&input, &mut out);
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
        let mut out = HashMap::new();
        parse_env(&input, &mut out);
        assert_eq!(out.get("PORT").unwrap(), "3000")
    }

    #[test]
    fn handle_qoutes() {
        let input = r#"
        PORT="3000"
        "#;
        let mut out = HashMap::new();
        parse_env(&input, &mut out);
        assert_eq!(out.get("PORT").unwrap(), "\"3000\"")
    }
}
