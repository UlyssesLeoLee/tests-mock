//! Mock backend 生命周期 trait。
//!
//! docs-only 阶段 trait method 体均为 `unimplemented!()`；
//! Phase 2 子代理 B 接管后会补 in-process / docker 两种模式的实装。

#![allow(async_fn_in_trait)]

use crate::config::MockConfig;
use crate::error::MockResult;

/// 生命周期句柄，记录 backend 启动信息
#[derive(Debug, Clone)]
pub struct LifecycleHandle {
    pub backend: String,
    pub started_at_unix_ms: i64,
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
}
