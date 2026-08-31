//! tests-mock-ai
//!
//! 模拟 AI provider（OpenAI / Anthropic / DeepSeek / 自研）的 5 个核心行为：
//! `complete` / `embed` / `stream_token` / `cancel` / `usage_stats`
//!
//! Phase 2 实装：in-process 内存实现。
//! Docker 模式（llama.cpp / vLLM）留 V1+。

#![forbid(unsafe_code)]
#![allow(async_fn_in_trait)]

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tests_mock_core::error::{MockError, MockResult};

/// Embedding 维度（mock 一律返回零向量）
const EMBED_DIM: usize = 768;

/// 默认模型名
const DEFAULT_MODEL: &str = "gpt-4o-mini";

/// Token 流句柄：用于 stream_token 取消 / 进度跟踪
#[derive(Debug, Clone)]
pub struct TokenStream {
    pub request_id: String,
    pub model: String,
    pub tokens: Vec<String>,
}

impl TokenStream {
    pub fn new(request_id: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            model: model.into(),
            tokens: Vec::new(),
        }
    }
}

/// 用量报告
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UsageReport {
    pub total_requests: u64,
    pub total_tokens: u64,
    pub total_cost_usd_micros: u64,
    pub by_model: HashMap<String, u64>,
}

/// AI mock 内部统计状态
#[derive(Default)]
struct AiState {
    cancelled: HashSet<String>,
    completions: u64,
    total_tokens: u64,
    total_cost_micros: u64,
    by_model: HashMap<String, u64>,
}

/// 解析 fixtures/ai_response_cache.json（不依赖 tests-mock-fixtures crate）
fn read_ai_response_cache() -> MockResult<Vec<AiResponseEntry>> {
    // 仓布局: <workspace>/crates/tests-mock-ai/Cargo.toml
    //   ->   <workspace>/crates/tests-mock-fixtures/fixtures/ai_response_cache.json
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir
        .parent()
        .map(|p| p.join("tests-mock-fixtures/fixtures/ai_response_cache.json"))
        .ok_or_else(|| MockError::Backend {
            backend: "ai".to_string(),
            message: "cannot resolve workspace root from CARGO_MANIFEST_DIR".to_string(),
        })?;
    let raw = std::fs::read_to_string(&path).map_err(|e| MockError::Backend {
        backend: "ai".to_string(),
        message: format!("read {}: {e}", path.display()),
    })?;
    #[derive(serde::Deserialize)]
    struct Wrapper {
        responses: Vec<AiResponseEntry>,
    }
    let w: Wrapper = serde_json::from_str(&raw).map_err(|e| MockError::Backend {
        backend: "ai".to_string(),
        message: format!("parse ai_response_cache.json: {e}"),
    })?;
    Ok(w.responses)
}

#[derive(Debug, Clone, serde::Deserialize)]
struct AiResponseEntry {
    #[allow(dead_code)]
    prompt_hash: String,
    model: String,
    completion: String,
    tokens: u64,
    #[allow(dead_code)]
    latency_ms: u64,
}

fn lock_err(e: impl std::fmt::Display) -> MockError {
    MockError::Backend {
        backend: "ai".to_string(),
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

fn now_unix_ms_i64() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 简单 FNV-1a 64-bit 哈希（确定性选择 cache 条目）
fn fnv1a_64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// AI mock 行为 trait
pub trait MockAi {
    /// 一次性完成（chat completion）
    async fn complete(&self, prompt: &str) -> MockResult<String>;

    /// 文本嵌入（embedding）
    async fn embed(&self, text: &str) -> MockResult<Vec<f32>>;

    /// 流式 token（返回 TokenStream 句柄）
    async fn stream_token(&self, prompt: &str) -> MockResult<TokenStream>;

    /// 取消进行中的请求
    async fn cancel(&self, request_id: &str) -> MockResult<()>;

    /// 用量统计（since_unix_ms 之后）
    async fn usage_stats(&self, since_unix_ms: i64) -> MockResult<UsageReport>;
}

pub type AiError = MockError;

/// In-memory AI mock 后端
#[derive(Clone)]
pub struct InMemoryAi {
    inner: Arc<Mutex<AiState>>,
}

impl InMemoryAi {
    /// 新建一个空的 in-process AI mock
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AiState::default())),
        }
    }

    /// 当前完成请求计数（仅测试 / 观测用）
    pub fn completions(&self) -> u64 {
        self.inner.lock().map(|s| s.completions).unwrap_or(0)
    }
}

impl Default for InMemoryAi {
    fn default() -> Self {
        Self::new()
    }
}

impl MockAi for InMemoryAi {
    async fn complete(&self, prompt: &str) -> MockResult<String> {
        require_non_empty("prompt", prompt)?;
        // 测试钩子：prompt 含 "timeout" 关键字 → 触发 timeout
        if prompt.contains("timeout") {
            return Err(MockError::Timeout {
                backend: "ai".to_string(),
                op: "complete".to_string(),
            });
        }
        // 优先从 fixture cache 选（按 prompt 哈希）
        let cache = read_ai_response_cache().ok();
        let (completion, model, tokens) = match cache {
            Some(ref entries) if !entries.is_empty() => {
                let idx = (fnv1a_64(prompt) as usize) % entries.len();
                let r = &entries[idx];
                (r.completion.clone(), r.model.clone(), r.tokens)
            }
            _ => {
                let preview_len = prompt.chars().take(40).count();
                let preview: String = prompt.chars().take(40).collect();
                (
                    format!("Mock completion for: {preview}"),
                    DEFAULT_MODEL.to_string(),
                    preview_len as u64,
                )
            }
        };
        let mut state = self.inner.lock().map_err(lock_err)?;
        state.completions += 1;
        state.total_tokens += tokens;
        // mock 计价：1 token = 1 micro USD
        state.total_cost_micros += tokens;
        *state.by_model.entry(model).or_insert(0) += tokens;
        Ok(completion)
    }

    async fn embed(&self, text: &str) -> MockResult<Vec<f32>> {
        require_non_empty("text", text)?;
        Ok(vec![0.0_f32; EMBED_DIM])
    }

    async fn stream_token(&self, prompt: &str) -> MockResult<TokenStream> {
        require_non_empty("prompt", prompt)?;
        let request_id = format!("req-{:x}", now_unix_ms_i64());
        let model = DEFAULT_MODEL.to_string();
        let tokens: Vec<String> = prompt
            .split_whitespace()
            .map(|w| {
                // 单词级 yield：每词去除标点，简化下游消费
                w.trim_end_matches(|c: char| !c.is_alphanumeric())
                    .to_string()
            })
            .filter(|t| !t.is_empty())
            .collect();
        let mut s = TokenStream::new(request_id, model);
        s.tokens = tokens;
        Ok(s)
    }

    async fn cancel(&self, request_id: &str) -> MockResult<()> {
        require_non_empty("request_id", request_id)?;
        let mut state = self.inner.lock().map_err(lock_err)?;
        if state.cancelled.insert(request_id.to_string()) {
            Ok(())
        } else {
            Err(MockError::AlreadyExists {
                resource: format!("cancelled:{request_id}"),
            })
        }
    }

    async fn usage_stats(&self, _since_unix_ms: i64) -> MockResult<UsageReport> {
        let state = self.inner.lock().map_err(lock_err)?;
        Ok(UsageReport {
            total_requests: state.completions,
            total_tokens: state.total_tokens,
            total_cost_usd_micros: state.total_cost_micros,
            by_model: state.by_model.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_error_alias_points_to_mock_error() {
        let e: AiError = MockError::Cancelled {
            request_id: "req-1234".to_string(),
        };
        assert!(matches!(e, MockError::Cancelled { .. }));
    }

    #[test]
    fn token_stream_constructor_sets_fields() {
        let s = TokenStream::new("req-1", "gpt-4o-mini");
        assert_eq!(s.request_id, "req-1");
        assert_eq!(s.model, "gpt-4o-mini");
        assert!(s.tokens.is_empty());
    }

    #[test]
    fn usage_report_serializes_roundtrip() {
        let mut by_model = std::collections::HashMap::new();
        by_model.insert("gpt-4o-mini".to_string(), 1024u64);
        let r = UsageReport {
            total_requests: 7,
            total_tokens: 1024,
            total_cost_usd_micros: 512,
            by_model,
        };
        let s = serde_json::to_string(&r).expect("serialize");
        let back: UsageReport = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(back.total_requests, 7);
        assert_eq!(back.total_tokens, 1024);
        assert_eq!(back.by_model.get("gpt-4o-mini"), Some(&1024));
    }

    #[tokio::test]
    async fn complete_returns_deterministic_cached_response() {
        let ai = InMemoryAi::new();
        let r1 = ai.complete("hello world").await.unwrap();
        let r2 = ai.complete("hello world").await.unwrap();
        // 同一 prompt 命中同一条 cache → 一致
        assert_eq!(r1, r2);
        assert!(!r1.is_empty());
        assert_eq!(ai.completions(), 2);
    }

    #[tokio::test]
    async fn complete_with_timeout_keyword_errors() {
        let ai = InMemoryAi::new();
        let res = ai.complete("please trigger timeout now").await;
        assert!(matches!(res, Err(MockError::Timeout { .. })));
    }

    #[tokio::test]
    async fn embed_returns_768_dim_zero_vector() {
        let ai = InMemoryAi::new();
        let v = ai.embed("hi").await.unwrap();
        assert_eq!(v.len(), 768);
        assert!(v.iter().all(|&x| x == 0.0));
    }

    #[tokio::test]
    async fn stream_token_yields_word_level_tokens() {
        let ai = InMemoryAi::new();
        let s = ai.stream_token("hello brave world").await.unwrap();
        assert!(!s.request_id.is_empty());
        assert_eq!(s.tokens, vec!["hello", "brave", "world"]);
    }

    #[tokio::test]
    async fn cancel_marks_request_id_once() {
        let ai = InMemoryAi::new();
        ai.cancel("req-aaa").await.unwrap();
        let res = ai.cancel("req-aaa").await;
        assert!(matches!(res, Err(MockError::AlreadyExists { .. })));
    }
}
