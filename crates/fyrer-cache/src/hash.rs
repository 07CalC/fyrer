use std::io::Read;

pub type CacheKey = String;
pub type OutputDigest = String;

pub fn hash_kv(hasher: &mut blake3::Hasher, key: &str, value: &str) {
    hasher.update(key.as_bytes());
    hasher.update(b"=");
    hasher.update(value.as_bytes());
    hasher.update(b"\n");
}

pub fn hash_file(hasher: &mut blake3::Hasher, path: &std::path::Path) -> std::io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = [0; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(())
}
