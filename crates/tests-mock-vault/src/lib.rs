//! tests-mock-vault
//!
//! 模拟 Credential Vault（HashiCorp Vault / 自研 VaultCache）的 5 个核心行为：
//! `get` / `set` / `delete` / `list` / `rotate`
//!
//! Phase 2 实装：in-process 内存实现（`Arc<Mutex<HashMap<...>>>`）。
//! Docker 模式留 V1+。

#![forbid(unsafe_code)]
#![allow(async_fn_in_trait)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tests_mock_core::error::{MockError, MockResult};

/// Vault mock 行为 trait
pub trait MockVault {
    /// 读取 secret；返回 `None` 表示 key 不存在（区分于 backend 错误）
    async fn get(&self, key: &str) -> MockResult<Option<String>>;

    /// 写入 / 覆盖 secret
    async fn set(&self, key: &str, value: &str) -> MockResult<()>;

    /// 删除 secret
    async fn delete(&self, key: &str) -> MockResult<()>;

    /// 列举所有 key
    async fn list(&self) -> MockResult<Vec<String>>;

    /// 轮转 secret（生成新 value，旧 value 保留 grace period）
    async fn rotate(&self, key: &str) -> MockResult<()>;
}

pub type VaultError = MockError;

fn lock_err(e: impl std::fmt::Display) -> MockError {
    MockError::Backend {
        backend: "vault".to_string(),
        message: format!("poisoned lock: {e}"),
    }
}

fn require_non_empty(field: &str, value: &str) -> MockResult<()> {
    if value.is_empty() {
        Err(MockError::InvalidInput {
            message: format!("{field} must not be empty"),
        })
    } else {
        Ok(())
    }
}

fn now_unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// In-memory Vault mock 后端
#[derive(Clone)]
pub struct InMemoryVault {
    inner: Arc<Mutex<HashMap<String, String>>>,
}

impl InMemoryVault {
    /// 新建一个空的 in-process Vault mock
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 当前 secret 条目数（仅测试 / 观测用）
    pub fn len(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }

    /// 是否为空（仅测试 / 观测用）
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for InMemoryVault {
    fn default() -> Self {
        Self::new()
    }
}

impl MockVault for InMemoryVault {
    async fn get(&self, key: &str) -> MockResult<Option<String>> {
        require_non_empty("key", key)?;
        let map = self.inner.lock().map_err(lock_err)?;
        Ok(map.get(key).cloned())
    }

    async fn set(&self, key: &str, value: &str) -> MockResult<()> {
        require_non_empty("key", key)?;
        require_non_empty("value", value)?;
        let mut map = self.inner.lock().map_err(lock_err)?;
        map.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn delete(&self, key: &str) -> MockResult<()> {
        require_non_empty("key", key)?;
        let mut map = self.inner.lock().map_err(lock_err)?;
        // 幂等：缺失 key 也返回 Ok(())
        map.remove(key);
        Ok(())
    }

    async fn list(&self) -> MockResult<Vec<String>> {
        let map = self.inner.lock().map_err(lock_err)?;
        let mut keys: Vec<String> = map.keys().cloned().collect();
        keys.sort();
        Ok(keys)
    }

    async fn rotate(&self, key: &str) -> MockResult<()> {
        require_non_empty("key", key)?;
        let mut map = self.inner.lock().map_err(lock_err)?;
        if !map.contains_key(key) {
            return Err(MockError::NotFound {
                resource: format!("vault:{key}"),
            });
        }
        let ts = now_unix_nanos();
        map.insert(key.to_string(), format!("rotated-{ts}"));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_error_alias_points_to_mock_error() {
        let e: VaultError = MockError::Backend {
            backend: "vault".to_string(),
            message: "lock contention".to_string(),
        };
        let s = format!("{e}");
        assert!(s.contains("vault"));
        assert!(s.contains("lock contention"));
    }

    #[tokio::test]
    async fn get_set_delete_roundtrip() {
        let v = InMemoryVault::new();
        assert!(v.get("k").await.unwrap().is_none());
        v.set("k", "v1").await.unwrap();
        assert_eq!(v.get("k").await.unwrap().as_deref(), Some("v1"));
        v.set("k", "v2").await.unwrap();
        assert_eq!(v.get("k").await.unwrap().as_deref(), Some("v2"));
        v.delete("k").await.unwrap();
        assert!(v.get("k").await.unwrap().is_none());
        // delete 不存在的 key 也 Ok（幂等）
        v.delete("ghost").await.unwrap();
    }

    #[tokio::test]
    async fn list_returns_sorted_keys() {
        let v = InMemoryVault::new();
        v.set("b", "2").await.unwrap();
        v.set("a", "1").await.unwrap();
        v.set("c", "3").await.unwrap();
        let keys = v.list().await.unwrap();
        assert_eq!(keys, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    #[tokio::test]
    async fn rotate_overwrites_value_with_timestamp() {
        let v = InMemoryVault::new();
        v.set("k", "old").await.unwrap();
        v.rotate("k").await.unwrap();
        let after = v.get("k").await.unwrap().unwrap();
        assert!(after.starts_with("rotated-"));
        assert_ne!(after, "old");
        let res = v.rotate("ghost").await;
        assert!(matches!(res, Err(MockError::NotFound { .. })));
    }
}
