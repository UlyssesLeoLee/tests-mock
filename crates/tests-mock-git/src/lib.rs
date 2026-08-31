//! tests-mock-git
//!
//! 模拟 Git server（裸仓库服务 / gitea / local git daemon）的 5 个核心行为：
//! `init_bare` / `receive_pack` / `upload_pack` / `get_refs` / `list_refs`
//!
//! Phase 2 实装：in-process 内存实现（`Arc<Mutex<BTreeMap<...>>>`）。
//! Docker 模式留 V1+。

#![forbid(unsafe_code)]
#![allow(async_fn_in_trait)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
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

/// 单仓库内部状态
#[derive(Default, Clone, Debug)]
struct GitRepoState {
    /// ref name -> target (sha or "ref: ...")
    refs: BTreeMap<String, String>,
    /// 最近一次 receive_pack 写入的字节流
    pack: Vec<u8>,
}

fn lock_err(e: impl std::fmt::Display) -> MockError {
    MockError::Backend {
        backend: "git".to_string(),
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

/// 极简 glob 匹配：支持 `*` (0+ 任意字符) 与 `?` (1 任意字符)
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_match_rec(&p, &t)
}

fn glob_match_rec(p: &[char], t: &[char]) -> bool {
    let mut pi = 0usize;
    let mut ti = 0usize;
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, ti));
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// In-memory Git mock 后端
#[derive(Clone)]
pub struct InMemoryGit {
    inner: Arc<Mutex<BTreeMap<String, GitRepoState>>>,
}

impl InMemoryGit {
    /// 新建一个空的 in-process Git mock
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// 当前 repo 数（仅测试 / 观测用）
    pub fn repo_count(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }
}

impl Default for InMemoryGit {
    fn default() -> Self {
        Self::new()
    }
}

impl MockGit for InMemoryGit {
    async fn init_bare(&self, path: &str) -> MockResult<()> {
        require_non_empty("path", path)?;
        let mut map = self.inner.lock().map_err(lock_err)?;
        if map.contains_key(path) {
            return Err(MockError::AlreadyExists {
                resource: format!("repo:{path}"),
            });
        }
        let mut repo = GitRepoState::default();
        repo.refs
            .insert("HEAD".to_string(), "ref: refs/heads/main".to_string());
        map.insert(path.to_string(), repo);
        Ok(())
    }

    async fn receive_pack(&self, repo: &str, data: &[u8]) -> MockResult<()> {
        if data.is_empty() {
            return Err(MockError::InvalidInput {
                message: "pkt-line data must not be empty".to_string(),
            });
        }
        let mut map = self.inner.lock().map_err(lock_err)?;
        let state = map.get_mut(repo).ok_or_else(|| MockError::NotFound {
            resource: format!("repo:{repo}"),
        })?;
        state.pack.extend_from_slice(data);
        Ok(())
    }

    async fn upload_pack(&self, repo: &str, wants: &[String]) -> MockResult<Vec<u8>> {
        require_non_empty("repo", repo)?;
        if wants.is_empty() {
            return Err(MockError::InvalidInput {
                message: "wants must not be empty".to_string(),
            });
        }
        let map = self.inner.lock().map_err(lock_err)?;
        let state = map.get(repo).ok_or_else(|| MockError::NotFound {
            resource: format!("repo:{repo}"),
        })?;
        for w in wants {
            if !state.refs.contains_key(w) && !state.refs.values().any(|v| v == w) {
                return Err(MockError::NotFound {
                    resource: format!("want:{w}"),
                });
            }
        }
        Ok(state.pack.clone())
    }

    async fn get_refs(&self, repo: &str) -> MockResult<Vec<(String, String)>> {
        require_non_empty("repo", repo)?;
        let map = self.inner.lock().map_err(lock_err)?;
        let state = map.get(repo).ok_or_else(|| MockError::NotFound {
            resource: format!("repo:{repo}"),
        })?;
        Ok(state.refs.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    async fn list_refs(&self, repo: &str, pattern: &str) -> MockResult<Vec<String>> {
        require_non_empty("repo", repo)?;
        let map = self.inner.lock().map_err(lock_err)?;
        let state = map.get(repo).ok_or_else(|| MockError::NotFound {
            resource: format!("repo:{repo}"),
        })?;
        let mut refs: Vec<String> = state
            .refs
            .keys()
            .filter(|k| glob_match(pattern, k))
            .cloned()
            .collect();
        refs.sort();
        Ok(refs)
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

    #[test]
    fn glob_match_handles_star_and_question_mark() {
        assert!(glob_match("refs/heads/*", "refs/heads/main"));
        assert!(glob_match("refs/heads/?ain", "refs/heads/main"));
        assert!(!glob_match("refs/heads/main", "refs/heads/dev"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("", ""));
        assert!(!glob_match("?", ""));
    }

    #[tokio::test]
    async fn init_bare_creates_repo_and_seeds_head() {
        let g = InMemoryGit::new();
        g.init_bare("/r1").await.unwrap();
        let refs = g.get_refs("/r1").await.unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, "HEAD");
    }

    #[tokio::test]
    async fn init_bare_twice_returns_already_exists() {
        let g = InMemoryGit::new();
        g.init_bare("/r1").await.unwrap();
        let res = g.init_bare("/r1").await;
        assert!(matches!(res, Err(MockError::AlreadyExists { .. })));
    }

    #[tokio::test]
    async fn receive_then_upload_returns_bytes() {
        let g = InMemoryGit::new();
        g.init_bare("/r1").await.unwrap();
        g.receive_pack("/r1", b"PACKdata").await.unwrap();
        let pack = g.upload_pack("/r1", &["HEAD".to_string()]).await.unwrap();
        assert_eq!(pack, b"PACKdata");
    }

    #[tokio::test]
    async fn list_refs_filters_by_glob() {
        let g = InMemoryGit::new();
        g.init_bare("/r1").await.unwrap();
        // 在 scope 内注入 ref 种子数据，锁会在 scope 结束时自动释放
        {
            let mut state = g.inner.lock().unwrap();
            let r = state.get_mut("/r1").unwrap();
            r.refs
                .insert("refs/heads/main".to_string(), "abc".to_string());
            r.refs
                .insert("refs/heads/dev".to_string(), "def".to_string());
            r.refs
                .insert("refs/tags/v1".to_string(), "ghi".to_string());
        }
        let heads = g.list_refs("/r1", "refs/heads/*").await.unwrap();
        assert_eq!(
            heads,
            vec!["refs/heads/dev".to_string(), "refs/heads/main".to_string()]
        );
        let tags = g.list_refs("/r1", "refs/tags/*").await.unwrap();
        assert_eq!(tags, vec!["refs/tags/v1".to_string()]);
    }
}
