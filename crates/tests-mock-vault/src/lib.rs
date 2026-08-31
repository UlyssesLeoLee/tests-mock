//! tests-mock-vault
//!
//! 模拟 Credential Vault（HashiCorp Vault / 自研 VaultCache）的 5 个核心行为：
//! `get` / `set` / `delete` / `list` / `rotate`
//!
//! docs-only 阶段 trait method 体均为 `unimplemented!()`。
//! Phase 2 子代理 B 接管后会补 in-process 内存实现 + 可选 docker postgres 适配。

#![forbid(unsafe_code)]
#![allow(async_fn_in_trait)]

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

/// docs-only stub：Phase 2 实装
pub struct InMemoryVault;

impl InMemoryVault {
    pub fn new() -> Self {
        unimplemented!("Phase 2 worker 实装 in-process Vault mock")
    }
}

impl Default for InMemoryVault {
    fn default() -> Self {
        Self::new()
    }
}

impl MockVault for InMemoryVault {
    async fn get(&self, _key: &str) -> MockResult<Option<String>> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn set(&self, _key: &str, _value: &str) -> MockResult<()> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn delete(&self, _key: &str) -> MockResult<()> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn list(&self) -> MockResult<Vec<String>> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn rotate(&self, _key: &str) -> MockResult<()> {
        unimplemented!("Phase 2 worker 实装")
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
}
