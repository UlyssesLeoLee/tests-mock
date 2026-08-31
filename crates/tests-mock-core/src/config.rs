//! Mock backend 共享配置。

use serde::{Deserialize, Serialize};

/// Mock backend 运行模式
///
/// - `InProcess`：纯 in-process 内存实现，单测 / 压测首选
/// - `Docker`：通过 docker compose 起 fake minIO / postgres / git server
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MockMode {
    InProcess,
    Docker,
}

/// 跨 mock backend 共享的配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockConfig {
    pub mode: MockMode,
    /// 监听端口（Docker 模式下使用；InProcess 模式默认 0）
    pub port: u16,
    /// 进程 ID（Docker 模式下的容器 PID；InProcess 模式默认 None）
    pub pid: Option<u32>,
    pub s3_endpoint: Option<String>,
    pub vault_endpoint: Option<String>,
    pub git_endpoint: Option<String>,
    pub ai_endpoint: Option<String>,
    pub state_file: String,
    pub report_file: String,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            mode: MockMode::InProcess,
            port: 0,
            pid: None,
            s3_endpoint: None,
            vault_endpoint: None,
            git_endpoint: None,
            ai_endpoint: None,
            state_file: "/tmp/tests-mock-state.json".to_string(),
            report_file: "/tmp/tests-mock-smoke-report.json".to_string(),
        }
    }
}
