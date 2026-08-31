//! tests-mock-s3
//!
//! 模拟 minIO / AWS S3 的 5 个核心行为：
//! `head_bucket` / `put_object` / `get_object` / `list_objects` / `delete_object`
//!
//! Phase 2 实装：in-process 内存实现（`Arc<Mutex<HashMap<...>>>`），
//! 保证 stress test 100 并发场景下 thread-safe。
//! Docker 模式留 V1+。

#![forbid(unsafe_code)]
#![allow(async_fn_in_trait)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tests_mock_core::error::{MockError, MockResult};

/// S3 mock 行为 trait
type S3Store = HashMap<String, HashMap<String, Vec<u8>>>;

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

/// 提取 `Mutex` 锁失败时的统一 Backend 错误
fn lock_err<E: std::fmt::Display>(backend: &str, e: E) -> MockError {
    MockError::Backend {
        backend: backend.to_string(),
        message: format!("poisoned lock: {e}"),
    }
}

/// 校验非空字符串
fn require_non_empty(field: &str, value: &str) -> MockResult<()> {
    if value.is_empty() {
        Err(MockError::InvalidInput {
            message: format!("{field} must not be empty"),
        })
    } else {
        Ok(())
    }
}

/// In-memory S3 mock 后端
///
/// 内部以 `bucket -> (key -> body)` 嵌套 HashMap 存储，
/// 用 `Arc<Mutex<...>>` 保护并发访问。
#[derive(Clone)]
pub struct InMemoryS3 {
    inner: Arc<Mutex<S3Store>>,
}

impl InMemoryS3 {
    /// 新建一个空的 in-process S3 mock
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 当前 bucket 数（仅测试 / 观测用）
    pub fn bucket_count(&self) -> usize {
        self.inner
            .lock()
            .map(|m| m.len())
            .unwrap_or_else(|_| 0)
    }
}

impl Default for InMemoryS3 {
    fn default() -> Self {
        Self::new()
    }
}

impl MockS3 for InMemoryS3 {
    async fn head_bucket(&self, bucket: &str) -> MockResult<()> {
        require_non_empty("bucket", bucket)?;
        let map = self.inner.lock().map_err(|e| lock_err("s3", e))?;
        if map.contains_key(bucket) {
            Ok(())
        } else {
            Err(MockError::NotFound {
                resource: format!("bucket:{bucket}"),
            })
        }
    }

    async fn put_object(&self, bucket: &str, key: &str, body: &[u8]) -> MockResult<()> {
        require_non_empty("bucket", bucket)?;
        require_non_empty("key", key)?;
        let mut map = self.inner.lock().map_err(|e| lock_err("s3", e))?;
        map.entry(bucket.to_string())
            .or_default()
            .insert(key.to_string(), body.to_vec());
        Ok(())
    }

    async fn get_object(&self, bucket: &str, key: &str) -> MockResult<Vec<u8>> {
        require_non_empty("bucket", bucket)?;
        require_non_empty("key", key)?;
        let map = self.inner.lock().map_err(|e| lock_err("s3", e))?;
        let bucket_obj = map.get(bucket).ok_or_else(|| MockError::NotFound {
            resource: format!("bucket:{bucket}"),
        })?;
        bucket_obj
            .get(key)
            .cloned()
            .ok_or_else(|| MockError::NotFound {
                resource: format!("object:{bucket}/{key}"),
            })
    }

    async fn list_objects(&self, bucket: &str, prefix: &str) -> MockResult<Vec<String>> {
        require_non_empty("bucket", bucket)?;
        let map = self.inner.lock().map_err(|e| lock_err("s3", e))?;
        let bucket_obj = map.get(bucket).ok_or_else(|| MockError::NotFound {
            resource: format!("bucket:{bucket}"),
        })?;
        let mut keys: Vec<String> = bucket_obj
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        keys.sort();
        Ok(keys)
    }

    async fn delete_object(&self, bucket: &str, key: &str) -> MockResult<()> {
        require_non_empty("bucket", bucket)?;
        require_non_empty("key", key)?;
        let mut map = self.inner.lock().map_err(|e| lock_err("s3", e))?;
        let bucket_obj = map.get_mut(bucket).ok_or_else(|| MockError::NotFound {
            resource: format!("bucket:{bucket}"),
        })?;
        if bucket_obj.remove(key).is_some() {
            Ok(())
        } else {
            Err(MockError::NotFound {
                resource: format!("object:{bucket}/{key}"),
            })
        }
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

    #[tokio::test]
    async fn head_bucket_roundtrip_with_put() {
        let s3 = InMemoryS3::new();
        assert!(matches!(
            s3.head_bucket("b1").await,
            Err(MockError::NotFound { .. })
        ));
        s3.put_object("b1", "k1", b"hello").await.unwrap();
        assert!(s3.head_bucket("b1").await.is_ok());
    }

    #[tokio::test]
    async fn put_get_roundtrip_returns_original_bytes() {
        let s3 = InMemoryS3::new();
        s3.put_object("b", "k", b"abc").await.unwrap();
        let got = s3.get_object("b", "k").await.unwrap();
        assert_eq!(got, b"abc");
    }

    #[tokio::test]
    async fn get_missing_key_returns_not_found() {
        let s3 = InMemoryS3::new();
        s3.put_object("b", "exists", b"x").await.unwrap();
        let res = s3.get_object("b", "nope").await;
        assert!(matches!(res, Err(MockError::NotFound { .. })));
    }

    #[tokio::test]
    async fn list_objects_filters_by_prefix() {
        let s3 = InMemoryS3::new();
        s3.put_object("b", "logs/a", b"1").await.unwrap();
        s3.put_object("b", "logs/b", b"2").await.unwrap();
        s3.put_object("b", "data/c", b"3").await.unwrap();
        let logs = s3.list_objects("b", "logs/").await.unwrap();
        assert_eq!(logs, vec!["logs/a".to_string(), "logs/b".to_string()]);
        let all = s3.list_objects("b", "").await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn delete_object_removes_then_errors() {
        let s3 = InMemoryS3::new();
        s3.put_object("b", "k", b"x").await.unwrap();
        s3.delete_object("b", "k").await.unwrap();
        let res = s3.delete_object("b", "k").await;
        assert!(matches!(res, Err(MockError::NotFound { .. })));
        let res = s3.get_object("b", "k").await;
        assert!(matches!(res, Err(MockError::NotFound { .. })));
    }
}
