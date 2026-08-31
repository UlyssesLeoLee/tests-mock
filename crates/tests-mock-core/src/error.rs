//! 统一错误类型。所有 mock backend 都通过 `MockResult<T>` 暴露结果。

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockError {
    /// 资源不存在（bucket / key / ref / request 等）
    NotFound { resource: String },
    /// 资源已存在（重复 init_bare / rotate 已存在 key 等）
    AlreadyExists { resource: String },
    /// 输入校验失败（空 key / 越界 prefix 等）
    InvalidInput { message: String },
    /// 后端内部错误（s3 / vault / git / ai 各 mock 的内部异常）
    Backend { backend: String, message: String },
    /// 超时（健康检查 / receive_pack / stream_token 等）
    Timeout { backend: String, op: String },
    /// 请求被取消（cancel 接口）
    Cancelled { request_id: String },
    /// 尚未实装（docs-only 阶段 trait method 命中）
    Unimplemented { feature: String },
    /// 兜底
    Other(String),
}

pub type MockResult<T> = Result<T, MockError>;

impl fmt::Display for MockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { resource } => write!(f, "NotFound: {resource}"),
            Self::AlreadyExists { resource } => write!(f, "AlreadyExists: {resource}"),
            Self::InvalidInput { message } => write!(f, "InvalidInput: {message}"),
            Self::Backend { backend, message } => write!(f, "{backend} backend: {message}"),
            Self::Timeout { backend, op } => write!(f, "{backend} {op} timeout"),
            Self::Cancelled { request_id } => write!(f, "request {request_id} cancelled"),
            Self::Unimplemented { feature } => write!(f, "Unimplemented: {feature}"),
            Self::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for MockError {}
