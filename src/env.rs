use std::collections::HashMap;

pub fn merge_env(
    global: &HashMap<String, String>,
    project: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged = global.clone();
    merged.extend(project.clone());
    merged
}
