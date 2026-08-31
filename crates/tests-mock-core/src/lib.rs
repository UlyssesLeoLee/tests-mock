//! tests-mock-core
//!
//! 跨项目 mock 脚手架的共享底盘：error / config / lifecycle。
//! 不实装任何 mock backend 行为，仅暴露 trait + 类型 + helper。

#![forbid(unsafe_code)]

pub mod error;
pub mod config;
pub mod lifecycle;

pub use error::{MockError, MockResult};
pub use config::{MockConfig, MockMode};
pub use lifecycle::{cleanup_mock_env, init_mock_env, now_unix_ms, LifecycleHandle, MockLifecycle};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_includes_kind() {
        let e = MockError::NotFound {
            resource: "bucket:demo".to_string(),
        };
        let s = format!("{e}");
        assert!(s.contains("NotFound"));
        assert!(s.contains("bucket:demo"));
    }

    #[test]
    fn error_display_for_backend_includes_backend() {
        let e = MockError::Backend {
            backend: "s3".to_string(),
            message: "connection refused".to_string(),
        };
        let s = format!("{e}");
        assert!(s.contains("s3"));
        assert!(s.contains("connection refused"));
    }

    #[test]
    fn error_is_std_error() {
        fn assert_error<E: std::error::Error>(_: &E) {}
        let e = MockError::Other("x".to_string());
        assert_error(&e);
    }

    #[test]
    fn config_roundtrip_json() {
        let cfg = MockConfig {
            mode: MockMode::InProcess,
            port: 0,
            pid: None,
            state_file: "/tmp/tests-mock-state.json".to_string(),
            report_file: "/tmp/tests-mock-smoke-report.json".to_string(),
            ..MockConfig::default()
        };
        let s = serde_json::to_string(&cfg).expect("serialize");
        let back: MockConfig = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(back.mode, MockMode::InProcess);
        assert_eq!(back.state_file, "/tmp/tests-mock-state.json");
        assert_eq!(back.port, 0);
        assert!(back.pid.is_none());
    }

    #[test]
    fn config_default_is_in_process_with_zero_port() {
        let cfg = MockConfig::default();
        assert_eq!(cfg.mode, MockMode::InProcess);
        assert_eq!(cfg.port, 0);
        assert!(cfg.pid.is_none());
    }

    #[tokio::test]
    async fn init_mock_env_returns_handle_with_current_unix_ms() {
        let cfg = MockConfig::default();
        let before = now_unix_ms();
        let h = init_mock_env(&cfg).await.expect("init");
        let after = now_unix_ms();
        assert_eq!(h.backend, "tests-mock");
        assert!(h.started_at_unix_ms >= before);
        assert!(h.started_at_unix_ms <= after);
    }

    #[tokio::test]
    async fn init_mock_env_rejects_empty_state_file() {
        let cfg = MockConfig {
            state_file: String::new(),
            ..MockConfig::default()
        };
        let res = init_mock_env(&cfg).await;
        assert!(matches!(res, Err(MockError::InvalidInput { .. })));
    }

    #[tokio::test]
    async fn cleanup_mock_env_is_idempotent() {
        let cfg = MockConfig::default();
        cleanup_mock_env(&cfg).await.expect("first cleanup");
        cleanup_mock_env(&cfg).await.expect("second cleanup");
    }
}
