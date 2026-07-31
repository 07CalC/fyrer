use std::collections::HashMap;

pub type EnvMap = HashMap<String, String>;

pub fn merge(base: &EnvMap, overrides: &EnvMap) -> EnvMap {
    let mut merged = base.clone();
    merged.extend(overrides.clone());
    merged
}
