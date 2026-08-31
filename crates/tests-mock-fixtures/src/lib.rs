//! tests-mock-fixtures
//!
//! JSON fixtures + loader helper.
//! Phase 2 子代理 C 接管后会补：
//! - `fixtures/user_creds.json`
//! - `fixtures/repo_metadata.json`
//! - `fixtures/ai_response_cache.json`
//! - `load_user_creds()` / `load_repo_metadata()` / `load_ai_response_cache()`
//!
//! 当前 commit 1 仅定义 schema 草稿 + 编译期常量。

#![forbid(unsafe_code)]
#![allow(dead_code)]

/// 当前 fixtures schema 版本
pub const FIXTURES_VERSION: &str = "v0.1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixtures_version_constant_is_v0_1() {
        assert_eq!(FIXTURES_VERSION, "v0.1");
    }
}
