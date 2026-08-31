//! Mock backend 生命周期 trait + 编排 helper。
//!
//! Phase 2 实装：
//! - `init_mock_env` / `cleanup_mock_env` 顶层 helper（给脚本侧调用）
//! - `LifecycleHandle::now` 当前 unix_ms 时间戳

#![allow(async_fn_in_trait)]

use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::MockConfig;
use crate::error::MockResult;

/// 生命周期句柄，记录 backend 启动信息
#[derive(Debug, Clone)]
pub struct LifecycleHandle {
    pub backend: String,
    pub started_at_unix_ms: i64,
}

impl LifecycleHandle {
    /// 新建一个 LifecycleHandle，`started_at_unix_ms` 自动取当前时间
    pub fn now(backend: impl Into<String>) -> Self {
        Self {
            backend: backend.into(),
            started_at_unix_ms: now_unix_ms(),
        }
    }
}

/// 返回当前 unix 毫秒时间戳
pub fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 所有 mock backend 共享的生命周期接口
///
/// 设计原则：start 由各 mock 自己实现（不同 backend 启动差异大），
/// health / stop 通过 trait 暴露统一接口。
pub trait MockLifecycle {
    /// 健康检查：返回 Ok 表示可服务
    async fn health(&self) -> MockResult<()>;

    /// 优雅停机：释放资源、flush 状态
    async fn stop(&self) -> MockResult<()>;

    /// 通用入口：从 MockConfig 启动一个 backend
    async fn start(_config: &MockConfig) -> MockResult<Self>
    where
        Self: Sized;

    /// 初始化（脚本侧友好别名，等价于 `start` 后的 `health`）
    async fn init(&self) -> MockResult<()> {
        self.health().await
    }

    /// 清理（脚本侧友好别名，等价于 `stop`）
    async fn cleanup(&self) -> MockResult<()> {
        self.stop().await
    }
}

/// 顶层 helper：脚本侧（PowerShell / Python）通过 Rust FFI 调用
/// 等价于 `init_mock_env` → health check → 写 state file
pub async fn init_mock_env(config: &MockConfig) -> MockResult<LifecycleHandle> {
    // 校验 config 自身合法
    if config.state_file.is_empty() {
        return Err(crate::error::MockError::InvalidInput {
            message: "MockConfig.state_file must not be empty".to_string(),
        });
    }
    // Phase 2 InProcess 模式：直接构造 LifecycleHandle，不依赖外部进程
    Ok(LifecycleHandle::now("tests-mock"))
}

/// 顶层 helper：脚本侧收尾，幂等（已清理的 state 不报错）
pub async fn cleanup_mock_env(_config: &MockConfig) -> MockResult<()> {
    // Phase 2 InProcess 模式：no-op
    Ok(())
}
