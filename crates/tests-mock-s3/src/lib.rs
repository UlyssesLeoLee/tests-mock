//! tests-mock-s3
//!
//! 模拟 minIO / AWS S3 的 5 个核心行为：
//! `head_bucket` / `put_object` / `get_object` / `list_objects` / `delete_object`
//!
//! docs-only 阶段 trait method 体均为 `unimplemented!()`。
//! Phase 2 子代理 B 接管后会补 in-process 内存实现 + 可选 docker minIO 适配。

#![forbid(unsafe_code)]
#![allow(async_fn_in_trait)]

use tests_mock_core::error::{MockError, MockResult};

/// S3 mock 行为 trait
pub trait MockS3 {
    /// 检查 bucket 是否存在
    async fn head_bucket(&self, bucket: &str) -> MockResult<()>;

    /// 写入对象（覆盖语义）
    async fn put_object(&self, bucket: &str, key: &str, body: &[u8]) -> MockResult<()>;

    /// 读取对象
    async fn get_object(&self, bucket: &str, key: &str) -> MockResult<Vec<u8>>;

    /// 列举对象（按 prefix 过滤）
    async fn list_objects(&self, bucket: &str, prefix: &str) -> MockResult<Vec<String>>;

    /// 删除对象
    async fn delete_object(&self, bucket: &str, key: &str) -> MockResult<()>;
}

/// 兼容别名：旧代码可直接 `use tests_mock_s3::S3Error`
pub type S3Error = MockError;

/// docs-only stub：Phase 2 实装
pub struct InMemoryS3;

impl InMemoryS3 {
    pub fn new() -> Self {
        unimplemented!("Phase 2 worker 实装 in-process S3 mock")
    }
}

impl Default for InMemoryS3 {
    fn default() -> Self {
        Self::new()
    }
}

impl MockS3 for InMemoryS3 {
    async fn head_bucket(&self, _bucket: &str) -> MockResult<()> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn put_object(&self, _bucket: &str, _key: &str, _body: &[u8]) -> MockResult<()> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn get_object(&self, _bucket: &str, _key: &str) -> MockResult<Vec<u8>> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn list_objects(&self, _bucket: &str, _prefix: &str) -> MockResult<Vec<String>> {
        unimplemented!("Phase 2 worker 实装")
    }
    async fn delete_object(&self, _bucket: &str, _key: &str) -> MockResult<()> {
        unimplemented!("Phase 2 worker 实装")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s3_error_alias_points_to_mock_error() {
        fn assert_send<E: Send + Sync + std::error::Error>(_: &E) {}
        let e: S3Error = MockError::NotFound {
            resource: "bucket:demo".to_string(),
        };
        assert_send(&e);
    }
}
