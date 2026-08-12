//! Shared library used by the `api` service.

pub fn greeting() -> String {
    "hello from acme-corp's shared Rust crate".to_string()
}