//! 会话凭据加密落盘 + OS 安全设施密钥（FR-002/004/005 / Clarification Q2 / research R3）。
//!
//! 设计：登录态从平台 WebView 提取为字节 blob → AES-GCM 加密 → 密文落盘 app data；
//! 加密密钥经 [`KeyProvider`] 存入 OS 安全设施（生产用 keyring）。明文绝不落盘。
//! 启动时解密回灌 WebView；断开（FR-004）时删密文 + 删密钥。

use std::path::PathBuf;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use rand::rngs::OsRng;
use rand::RngCore;

use crate::adapters::PlatformId;
use crate::error::{AppError, AppResult};

const KEYRING_SERVICE: &str = "quick-publish.session";

/// 加密密钥来源抽象：生产用 OS 安全设施，测试用内存，从而 [`SessionStore`] 可单测。
pub trait KeyProvider: Send + Sync {
    /// 取该平台的 32 字节密钥；不存在则创建并持久化。
    fn get_or_create_key(&self, platform: PlatformId) -> AppResult<[u8; 32]>;
    /// 删除该平台密钥（断开连接，FR-004）。
    fn delete_key(&self, platform: PlatformId) -> AppResult<()>;
}

/// 生产实现：密钥存 OS 安全设施（Windows Credential Manager / macOS Keychain / Linux Secret Service）。
pub struct OsKeyProvider;

impl OsKeyProvider {
    fn entry(platform: PlatformId) -> AppResult<keyring::Entry> {
        keyring::Entry::new(KEYRING_SERVICE, platform.as_str())
            .map_err(|e| AppError::Io(format!("keyring entry: {e}")))
    }
}

impl KeyProvider for OsKeyProvider {
    fn get_or_create_key(&self, platform: PlatformId) -> AppResult<[u8; 32]> {
        let entry = Self::entry(platform)?;
        match entry.get_password() {
            Ok(stored) => decode_key(&stored),
            Err(keyring::Error::NoEntry) => {
                let mut key = [0u8; 32];
                OsRng.fill_bytes(&mut key);
                entry
                    .set_password(&hex::encode(key))
                    .map_err(|e| AppError::Io(format!("keyring set: {e}")))?;
                Ok(key)
            }
            Err(e) => Err(AppError::Io(format!("keyring get: {e}"))),
        }
    }

    fn delete_key(&self, platform: PlatformId) -> AppResult<()> {
        let entry = Self::entry(platform)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Io(format!("keyring delete: {e}"))),
        }
    }
}

fn decode_key(hexstr: &str) -> AppResult<[u8; 32]> {
    let bytes = hex::decode(hexstr).map_err(|e| AppError::Invalid(format!("key hex: {e}")))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| AppError::Invalid("密钥长度非 32 字节".into()))?;
    Ok(arr)
}

/// 会话密文存储：每平台一个 `<dir>/<platform>.bin`，文件内容为 `nonce(12) || ciphertext`。
pub struct SessionStore<K: KeyProvider> {
    dir: PathBuf,
    keys: K,
}

impl<K: KeyProvider> SessionStore<K> {
    pub fn new(dir: impl Into<PathBuf>, keys: K) -> Self {
        SessionStore {
            dir: dir.into(),
            keys,
        }
    }

    fn blob_path(&self, platform: PlatformId) -> PathBuf {
        self.dir.join(format!("{}.bin", platform.as_str()))
    }

    /// 加密保存登录态 blob（FR-002/005）。明文不落盘。
    pub fn save(&self, platform: PlatformId, plaintext: &[u8]) -> AppResult<()> {
        std::fs::create_dir_all(&self.dir)?;
        let key = self.keys.get_or_create_key(platform)?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| AppError::Invalid(format!("cipher init: {e}")))?;
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| AppError::Invalid(format!("encrypt: {e}")))?;
        let mut out = Vec::with_capacity(12 + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        std::fs::write(self.blob_path(platform), out)?;
        Ok(())
    }

    /// 读取并解密登录态 blob；不存在返回 `None`。
    pub fn load(&self, platform: PlatformId) -> AppResult<Option<Vec<u8>>> {
        let path = self.blob_path(platform);
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read(&path)?;
        if raw.len() < 12 {
            return Err(AppError::Invalid("会话密文损坏".into()));
        }
        let key = self.keys.get_or_create_key(platform)?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| AppError::Invalid(format!("cipher init: {e}")))?;
        let (nonce_bytes, ciphertext) = raw.split_at(12);
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
            .map_err(|e| AppError::Auth(format!("会话解密失败（可能需重新登录）: {e}")))?;
        Ok(Some(plaintext))
    }

    /// 是否已保存该平台会话（用于推断连接状态）。
    pub fn exists(&self, platform: PlatformId) -> bool {
        self.blob_path(platform).exists()
    }

    /// 断开连接：删密文 + 删密钥（FR-004）。
    pub fn clear(&self, platform: PlatformId) -> AppResult<()> {
        let path = self.blob_path(platform);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        self.keys.delete_key(platform)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// 测试用内存密钥提供者（不触达 OS keyring）。
    struct MemKeyProvider {
        keys: Mutex<HashMap<PlatformId, [u8; 32]>>,
    }
    impl MemKeyProvider {
        fn new() -> Self {
            MemKeyProvider {
                keys: Mutex::new(HashMap::new()),
            }
        }
    }
    impl KeyProvider for MemKeyProvider {
        fn get_or_create_key(&self, platform: PlatformId) -> AppResult<[u8; 32]> {
            let mut m = self.keys.lock().unwrap();
            let k = m.entry(platform).or_insert_with(|| {
                let mut key = [0u8; 32];
                OsRng.fill_bytes(&mut key);
                key
            });
            Ok(*k)
        }
        fn delete_key(&self, platform: PlatformId) -> AppResult<()> {
            self.keys.lock().unwrap().remove(&platform);
            Ok(())
        }
    }

    fn tmp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let mut p = std::env::temp_dir();
        p.push(format!(
            "qp-session-test-{nanos:x}-{}",
            N.fetch_add(1, Ordering::Relaxed)
        ));
        p
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let dir = tmp_dir();
        let store = SessionStore::new(dir.clone(), MemKeyProvider::new());
        let secret = b"cookie=abc123; sid=xyz";
        store.save(PlatformId::Weixin, secret).unwrap();

        // 明文不落盘：密文文件内容不应包含原始明文片段
        let raw = std::fs::read(dir.join("weixin.bin")).unwrap();
        assert!(!raw.windows(secret.len()).any(|w| w == secret));

        let loaded = store.load(PlatformId::Weixin).unwrap().unwrap();
        assert_eq!(loaded, secret);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_absent_returns_none() {
        let store = SessionStore::new(tmp_dir(), MemKeyProvider::new());
        assert!(store.load(PlatformId::Zhihu).unwrap().is_none());
    }

    #[test]
    fn clear_removes_blob_and_key() {
        let dir = tmp_dir();
        let store = SessionStore::new(dir.clone(), MemKeyProvider::new());
        store.save(PlatformId::Juejin, b"x").unwrap();
        assert!(store.exists(PlatformId::Juejin));
        store.clear(PlatformId::Juejin).unwrap();
        assert!(!store.exists(PlatformId::Juejin));
        std::fs::remove_dir_all(&dir).ok();
    }
}
