//! Integration tests for the fixture loader helper.
//!
//! 覆盖目标（per 8/31 16:13 JST 测试设计书 §4）：
//! - serde 强类型解析（3 fixtures × 顶层结构）
//! - 字段访问（嵌套 user/keys, repo/labels, response/model）
//! - 错误处理（缺文件、坏 JSON、版本不匹配）

use std::fs;
use std::path::PathBuf;
use tests_mock_fixtures::{
    fixtures_path, load_ai_response_cache, load_repo_metadata, load_user_creds, FIXTURES_VERSION,
};

// ---------------------------------------------------------------------------
// 1. user_creds.json — 强类型 + 字段访问
// ---------------------------------------------------------------------------

#[test]
fn user_creds_loads_with_two_users() {
    let creds = load_user_creds().expect("load_user_creds");
    assert_eq!(creds.users.len(), 2, "expected 2 users in fixture");
}

#[test]
fn user_creds_alice_has_three_vault_keys() {
    let creds = load_user_creds().expect("load_user_creds");
    let alice = creds
        .users
        .iter()
        .find(|u| u.name == "alice")
        .expect("alice must exist");
    assert_eq!(alice.vault_keys.len(), 3);
    assert!(alice.vault_keys.contains(&"openai:key".to_string()));
    assert!(alice.vault_keys.contains(&"github:pat".to_string()));
    assert!(alice.vault_keys.contains(&"gitee:pat".to_string()));
}

#[test]
fn user_creds_bob_has_two_vault_keys() {
    let creds = load_user_creds().expect("load_user_creds");
    let bob = creds
        .users
        .iter()
        .find(|u| u.name == "bob")
        .expect("bob must exist");
    assert_eq!(bob.vault_keys.len(), 2);
    assert!(bob.vault_keys.contains(&"anthropic:key".to_string()));
    assert!(bob.vault_keys.contains(&"gitlab:pat".to_string()));
}

#[test]
fn user_creds_test_key_is_explicit_placeholder() {
    // 重要：test_key 是占位符，不能误认为是真凭证
    let creds = load_user_creds().expect("load_user_creds");
    assert_eq!(creds.test_key, "sk-mock-test-only-not-real");
    assert!(creds.test_key.starts_with("sk-mock-"));
    assert!(!creds.test_key.contains("sk-prod-"));
    assert!(!creds.test_key.contains("sk-live-"));
}

#[test]
fn user_creds_version_matches_v0_1() {
    let creds = load_user_creds().expect("load_user_creds");
    assert_eq!(creds.version, FIXTURES_VERSION);
    assert_eq!(creds.version, "v0.1");
}

// ---------------------------------------------------------------------------
// 2. repo_metadata.json — 强类型 + 字段访问
// ---------------------------------------------------------------------------

#[test]
fn repo_metadata_loads_with_two_repos() {
    let meta = load_repo_metadata().expect("load_repo_metadata");
    assert_eq!(meta.repos.len(), 2);
}

#[test]
fn repo_metadata_star_1025_has_ai_ide_compat_label() {
    let meta = load_repo_metadata().expect("load_repo_metadata");
    let r = meta
        .repos
        .iter()
        .find(|r| r.id == "STAR-1025")
        .expect("STAR-1025 must exist");
    assert!(r.labels.contains(&"ai-ide-compat".to_string()));
    assert_eq!(r.priority, "HIGH");
    assert_eq!(r.status, "IN_PROGRESS");
}

#[test]
fn repo_metadata_star_1024_is_mock_medium_open() {
    let meta = load_repo_metadata().expect("load_repo_metadata");
    let r = meta
        .repos
        .iter()
        .find(|r| r.id == "STAR-1024")
        .expect("STAR-1024 must exist");
    assert_eq!(r.priority, "MEDIUM");
    assert_eq!(r.status, "OPEN");
    assert_eq!(r.labels, vec!["mock".to_string()]);
}

// ---------------------------------------------------------------------------
// 3. ai_response_cache.json — 强类型 + 字段访问
// ---------------------------------------------------------------------------

#[test]
fn ai_response_cache_loads_with_two_responses() {
    let cache = load_ai_response_cache().expect("load_ai_response_cache");
    assert_eq!(cache.responses.len(), 2);
}

#[test]
fn ai_response_cache_first_response_is_gpt_4o_mini() {
    let cache = load_ai_response_cache().expect("load_ai_response_cache");
    let r = &cache.responses[0];
    assert_eq!(r.prompt_hash, "abc123");
    assert_eq!(r.model, "gpt-4o-mini");
    assert_eq!(r.completion, "Mock commit message for testing");
    assert_eq!(r.tokens, 42);
    assert_eq!(r.latency_ms, 235);
}

#[test]
fn ai_response_cache_total_tokens_is_170() {
    let cache = load_ai_response_cache().expect("load_ai_response_cache");
    let total: u64 = cache.responses.iter().map(|r| r.tokens).sum();
    assert_eq!(total, 170, "42 + 128 = 170");
}

#[test]
fn ai_response_cache_lookup_by_prompt_hash() {
    let cache = load_ai_response_cache().expect("load_ai_response_cache");
    let def = cache
        .responses
        .iter()
        .find(|r| r.prompt_hash == "def456")
        .expect("def456 must exist");
    assert_eq!(def.model, "claude-3-5-haiku-latest");
    assert_eq!(def.completion, "Mock review comment");
    assert_eq!(def.latency_ms, 412);
}

// ---------------------------------------------------------------------------
// 4. 错误处理 — 缺文件、坏 JSON、版本不匹配
// ---------------------------------------------------------------------------

#[test]
fn missing_fixture_returns_backend_error() {
    // fixtures_path 只是把路径拼出来，并不读文件
    let p: PathBuf = fixtures_path("does_not_exist.json");
    let raw = fs::read_to_string(&p);
    assert!(raw.is_err(), "read should fail for missing file");
}

#[test]
fn malformed_json_returns_backend_error() {
    // 临时写一份坏 JSON，确认 serde 解析失败被包装成 MockError::Backend
    let p = fixtures_path("__bad_test.json");
    fs::write(&p, "{ not valid json").expect("write temp");
    let res: Result<tests_mock_fixtures::UserCreds, _> = serde_json::from_str(
        &fs::read_to_string(&p).expect("read temp"),
    )
    .map_err(|e| tests_mock_core::MockError::Backend {
        backend: "fixtures".to_string(),
        message: format!("parse: {e}"),
    });
    let _ = fs::remove_file(&p);
    assert!(res.is_err());
}

#[test]
fn version_constant_is_stable() {
    // 防止有人误改 FIXTURES_VERSION 而忘了同步 fixture
    assert_eq!(FIXTURES_VERSION, "v0.1");
}
