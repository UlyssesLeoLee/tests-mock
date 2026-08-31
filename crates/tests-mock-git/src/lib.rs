//! tests-mock-git
//!
//! 模拟 Git server（裸仓库服务 / gitea / local git daemon）的 5 个核心行为：
//! `init_bare` / `receive_pack` / `upload_pack` / `get_refs` / `list_refs`
//!
//! docs-only 阶段 trait method 体均为 `unimplemented!()`。
//! Phase 2 子代理 B 接管后会补 in-process 内存实现 + 可选 docker gitea 适配。

#![forbid(unsafe_code)]
#![allow(async_fn_in_trait)]

use tests_mock_core::error::{MockError, MockResult};

/// Git mock 行为 trait
pub trait MockGit {
    /// 初始化裸仓库；返回 Ok 表示成功
    async fn init_bare(&self, path: &str) -> MockResult<()>;

    /// 接收 push 数据（pkt-line 流）
    async fn receive_pack(&self, repo: &str, data: &[u8]) -> MockResult<()>;

    /// 处理 fetch / clone 请求
    async fn upload_pack(&self, repo: &str, wants: &[String]) -> MockResult<Vec<u8>>;

    /// 读取所有 ref
    async fn get_refs(&self, repo: &str) -> MockResult<Vec<(String, String)>>;

    /// 按 glob 模式过滤 ref
    async fn list_refs(&self, repo: &str, pattern: &str) -> MockResult<Vec<String>>;
}

pub type GitError = MockError;

/// docs-only stub：Phase 2 实装
pub struct InMemoryGit;

impl InMemoryGit {
    pub fn new() -> Self {
        unimplemented!("Phase 2 worker 实装 in-process Git mock")
    }
}

impl Default for InMemoryGit {
    fn default() -> Self {
        Self::new()
    }
}

impl MockGit for InMemoryGit {
    async fn init_bare(&self, _path: &str) -> MockResult<()> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn receive_pack(&self, _repo: &str, _data: &[u8]) -> MockResult<()> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn upload_pack(&self, _repo: &str, _wants: &[String]) -> MockResult<Vec<u8>> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn get_refs(&self, _repo: &str) -> MockResult<Vec<(String, String)>> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn list_refs(&self, _repo: &str, _pattern: &str) -> MockResult<Vec<String>> {
        unimplemented!("Phase 2 worker 实装")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_error_alias_points_to_mock_error() {
        let e: GitError = MockError::AlreadyExists {
            resource: "repo:test.git".to_string(),
        };
        assert!(matches!(e, MockError::AlreadyExists { .. }));
    }
}
