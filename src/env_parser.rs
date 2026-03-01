use std::collections::HashMap;

pub fn parse_env(content: &str) -> HashMap<String, String> {
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
