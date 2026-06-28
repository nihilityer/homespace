//! 内嵌密码哈希 — 用于生成 `Traefik` basicAuth 所需的 htpasswd 格式凭据。
//! 使用 `{SHA}` 格式（SHA-1 哈希 + Base64 编码）。

use base64::Engine;
use sha1::{Digest, Sha1};

/// 生成一条 htpasswd 格式的用户记录（`{SHA}` 哈希）。
///
/// 返回 `username:{SHA}base64hash`，可直接写入 `users.txt`。
#[must_use]
pub fn htpasswd_sha1_entry(username: &str, password: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(password.as_bytes());
    let hash = hasher.finalize();
    let b64 = base64::engine::general_purpose::STANDARD.encode(hash);
    format!("{username}:{{SHA}}{b64}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_htpasswd_format() {
        let entry = htpasswd_sha1_entry("admin", "test123");
        assert!(entry.starts_with("admin:{SHA}"));
        // Base64 输出不含空格
        assert!(!entry.contains(' '));
    }
}
