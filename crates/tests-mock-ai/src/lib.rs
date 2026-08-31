//! tests-mock-ai
//!
//! 模拟 AI provider（OpenAI / Anthropic / DeepSeek / 自研）的 5 个核心行为：
//! `complete` / `embed` / `stream_token` / `cancel` / `usage_stats`
//!
//! docs-only 阶段 trait method 体均为 `unimplemented!()`。
//! Phase 2 子代理 B 接管后会补 in-process 内存实现 + 可选 docker llama.cpp 适配。

#![forbid(unsafe_code)]
#![allow(async_fn_in_trait)]

use tests_mock_core::error::{MockError, MockResult};

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
    pub by_model: std::collections::HashMap<String, u64>,
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

/// docs-only stub：Phase 2 实装
pub struct InMemoryAi;

impl InMemoryAi {
    pub fn new() -> Self {
        unimplemented!("Phase 2 worker 实装 in-process AI mock")
    }
}

impl Default for InMemoryAi {
    fn default() -> Self {
        Self::new()
    }
}

impl MockAi for InMemoryAi {
    async fn complete(&self, _prompt: &str) -> MockResult<String> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn embed(&self, _text: &str) -> MockResult<Vec<f32>> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn stream_token(&self, _prompt: &str) -> MockResult<TokenStream> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn cancel(&self, _request_id: &str) -> MockResult<()> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn usage_stats(&self, _since_unix_ms: i64) -> MockResult<UsageReport> {
        unimplemented!("Phase 2 worker 实装")
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
}
