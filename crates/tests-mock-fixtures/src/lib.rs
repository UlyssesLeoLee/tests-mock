//! tests-mock-fixtures
//!
//! 3 份 JSON fixture + loader helper。
//! docs-only Phase 1 仅提供 schema 强类型 + 文件 I/O；Phase 2 子代理 C 接管后会补：
//! - 注入 vault 内存 / git 裸仓库 / ai response cache 的回放
//!
//! ## Fixtures
//!
//! - `fixtures/user_creds.json` — vault 凭证用户名单（test_key 字段是占位符，**不是真凭证**）
//! - `fixtures/repo_metadata.json` — git mock 仓库元数据 + 关联 issue 列表
//! - `fixtures/ai_response_cache.json` — AI provider mock 离线回放缓存
//!
//! ## Loader API
//!
//! - [`load_user_creds`] → [`UserCreds`]
//! - [`load_repo_metadata`] → [`RepoMetadata`]
//! - [`load_ai_response_cache`] → [`AiResponseCache`]
//!
//! 所有 loader 都从 `CARGO_MANIFEST_DIR/fixtures/<filename>` 读取，并把
//! `serde_json::Error` / `std::io::Error` 包装成 `MockError::Backend`，
//! 方便下游统一处理。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tests_mock_core::error::{MockError, MockResult};

/// 当前 fixtures schema 版本
pub const FIXTURES_VERSION: &str = "v0.1";

// ---------------------------------------------------------------------------
// Types: UserCreds
// ---------------------------------------------------------------------------

/// 顶层 user_creds.json schema
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCreds {
    pub version: String,
    pub users: Vec<UserEntry>,
    /// **占位符** — 仅用于 mock backend 自检，**不是**真实凭证
    pub test_key: String,
}

/// 单个 user 条目
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserEntry {
    pub name: String,
    pub vault_keys: Vec<String>,
}

// ---------------------------------------------------------------------------
// Types: RepoMetadata
// ---------------------------------------------------------------------------

/// 顶层 repo_metadata.json schema
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoMetadata {
    pub version: String,
    pub repos: Vec<RepoEntry>,
}

/// 单个 repo / issue 条目
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoEntry {
    pub id: String,
    pub title: String,
    pub labels: Vec<String>,
    pub priority: String,
    pub status: String,
}

// ---------------------------------------------------------------------------
// Types: AiResponseCache
// ---------------------------------------------------------------------------

/// 顶层 ai_response_cache.json schema
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiResponseCache {
    pub version: String,
    pub responses: Vec<AiResponseEntry>,
}

/// 单条 AI 响应缓存条目
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiResponseEntry {
    pub prompt_hash: String,
    pub model: String,
    pub completion: String,
    pub tokens: u64,
    pub latency_ms: u64,
}

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------

/// 解析 fixtures/<filename> 到强类型 T，错误统一包装成 MockError::Backend
fn load_fixture<T>(name: &str) -> MockResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let path = fixtures_path(name);
    let raw = std::fs::read_to_string(&path).map_err(|e| MockError::Backend {
        backend: "fixtures".to_string(),
        message: format!("read {}: {e}", path.display()),
    })?;
    serde_json::from_str(&raw).map_err(|e| MockError::Backend {
        backend: "fixtures".to_string(),
        message: format!("parse {name}: {e}"),
    })
}

/// 解析 fixtures/<filename> 但额外校验顶层 `version` 等于 [`FIXTURES_VERSION`]
fn load_fixture_versioned<T>(name: &str) -> MockResult<T>
where
    T: for<'de> Deserialize<'de> + HasVersion,
{
    let value = load_fixture::<T>(name)?;
    if value.version() != FIXTURES_VERSION {
        return Err(MockError::Backend {
            backend: "fixtures".to_string(),
            message: format!(
                "{name} version mismatch: expected {FIXTURES_VERSION}, got {}",
                value.version()
            ),
        });
    }
    Ok(value)
}

trait HasVersion {
    fn version(&self) -> &str;
}

impl HasVersion for UserCreds {
    fn version(&self) -> &str {
        &self.version
    }
}
impl HasVersion for RepoMetadata {
    fn version(&self) -> &str {
        &self.version
    }
}
impl HasVersion for AiResponseCache {
    fn version(&self) -> &str {
        &self.version
    }
}

/// 加载 user_creds.json
pub fn load_user_creds() -> MockResult<UserCreds> {
    load_fixture_versioned("user_creds.json")
}

/// 加载 repo_metadata.json
pub fn load_repo_metadata() -> MockResult<RepoMetadata> {
    load_fixture_versioned("repo_metadata.json")
}

/// 加载 ai_response_cache.json
pub fn load_ai_response_cache() -> MockResult<AiResponseCache> {
    load_fixture_versioned("ai_response_cache.json")
}

/// 返回 fixtures/<filename> 的绝对路径
///
/// 暴露这个 helper 用于集成测试 + 脚本侧验证
pub fn fixtures_path(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    dir.join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixtures_version_constant_is_v0_1() {
        assert_eq!(FIXTURES_VERSION, "v0.1");
    }

    #[test]
    fn fixtures_path_resolves_under_manifest_dir() {
        let p = fixtures_path("user_creds.json");
        assert!(p.is_absolute(), "expected absolute path, got {}", p.display());
        assert!(p.ends_with(Path::new("fixtures").join("user_creds.json")));
    }

    #[test]
    fn load_user_creds_succeeds() {
        let creds = load_user_creds().expect("load");
        assert_eq!(creds.users.len(), 2);
        assert_eq!(creds.users[0].name, "alice");
    }

    #[test]
    fn load_repo_metadata_succeeds() {
        let meta = load_repo_metadata().expect("load");
        assert_eq!(meta.repos.len(), 2);
        assert_eq!(meta.repos[1].id, "STAR-1025");
    }

    #[test]
    fn load_ai_response_cache_succeeds() {
        let cache = load_ai_response_cache().expect("load");
        assert_eq!(cache.responses.len(), 2);
        assert_eq!(cache.responses[0].model, "gpt-4o-mini");
    }
}
