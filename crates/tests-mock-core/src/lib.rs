//! tests-mock-core
//!
//! 跨项目 mock 脚手架的共享底盘：error / config / lifecycle。
//! 不实装任何 mock backend 行为，仅暴露 trait + 类型。

#![forbid(unsafe_code)]

pub mod error;
pub mod config;
pub mod lifecycle;

pub use error::{MockError, MockResult};
pub use config::{MockConfig, MockMode};
pub use lifecycle::{LifecycleHandle, MockLifecycle};

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
            s3_endpoint: None,
            vault_endpoint: None,
            git_endpoint: None,
            ai_endpoint: None,
            state_file: "/tmp/tests-mock-state.json".to_string(),
            report_file: "/tmp/tests-mock-smoke-report.json".to_string(),
        };
        let s = serde_json::to_string(&cfg).expect("serialize");
        let back: MockConfig = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(back.mode, MockMode::InProcess);
        assert_eq!(back.state_file, "/tmp/tests-mock-state.json");
    }
}
