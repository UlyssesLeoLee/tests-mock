# tests-mock

> 跨项目测试 mock 脚手架 — Rust workspace

`tests-mock` 是与 `GitGit` / `RustGameServer` / `Physis` / `Star` **并列** 的独立跨项目测试脚手架。
本仓专门为**测试场景**提供 mock backend、fixture、脚本与设计书，**不实装生产逻辑**。

---

## 仓定位

| 维度 | 内容 |
| --- | --- |
| 角色 | 跨项目共享的 mock backend 仓（4 trait backend + 1 共享底盘；in-process + 可选 docker compose） |
| 集成方式 | 下游 Rust 项目 `cargo test --features tests-mock` 引用 |
| 设计原则 | 0 unsafe / 0 业务逻辑 / docs-only stub 阶段只暴露 trait |
| 远端 | `https://github.com/UlyssesLeoLee/tests-mock.git`（暂未 push per R-05） |
| 维护者 | Ulysses（一人公司 12 角色 per DEC-008）— Mavis 接手代签 |

---

## Workspace 结构

```
tests-mock/
├── Cargo.toml                          # workspace 根
├── crates/
│   ├── tests-mock-core/                # 核心 trait + error + config + lifecycle
│   ├── tests-mock-s3/                  # minIO / S3 mock (5 行为)
│   ├── tests-mock-vault/               # Credential Vault mock (5 行为)
│   ├── tests-mock-git/                 # Git server mock (5 行为)
│   ├── tests-mock-ai/                  # AI provider mock (5 行为)
│   └── tests-mock-fixtures/            # 3 份 JSON fixture + loader helper
├── docs/
│   └── TEST-DESIGN.md                  # 7 段测试设计书
├── scripts/                            # 5 mock 脚本 (PS + Python 双版本)
│   ├── init_mock_env.{ps1,py}
│   ├── seed_fixtures.{ps1,py}
│   ├── run_smoke_test.{ps1,py}
│   ├── stress_concurrency.{ps1,py}
│   └── cleanup_mock_env.{ps1,py}
└── crates/tests-mock-fixtures/fixtures/
    ├── user_creds.json
    ├── repo_metadata.json
    └── ai_response_cache.json
```

---

## Mock Backend 概览

4 个 trait mock backend + 1 个共享底盘（`tests-mock-core` 是 `error / config / lifecycle`
共享底盘，**不**是 mock backend，run_smoke_test 中作为 meta entry 单独统计）。

| Crate | 角色 | 5 个 mock 行为 |
| --- | --- | --- |
| `tests-mock-s3` | minIO / S3 mock backend | `head_bucket` / `put_object` / `get_object` / `list_objects` / `delete_object` |
| `tests-mock-vault` | Credential Vault mock backend | `get` / `set` / `delete` / `list` / `rotate` |
| `tests-mock-git` | Git server mock backend | `init_bare` / `receive_pack` / `upload_pack` / `get_refs` / `list_refs` |
| `tests-mock-ai` | AI provider mock backend | `complete` / `embed` / `stream_token` / `cancel` / `usage_stats` |
| `tests-mock-core` | 共享底盘（meta） | `MockError` / `MockConfig` / `MockLifecycle` / `init_mock_env` / `cleanup_mock_env` |

---

## 跨项目集成示例

### 方式 1：path 依赖（推荐开发期）

```toml
# 下游项目 Cargo.toml
[dev-dependencies]
tests-mock-s3 = { path = "D:/tests-mock/crates/tests-mock-s3" }
tests-mock-vault = { path = "D:/tests-mock/crates/tests-mock-vault" }
tests-mock-fixtures = { path = "D:/tests-mock/crates/tests-mock-fixtures" }
```

### 方式 2：feature gate（推荐生产集成）

```toml
# 下游项目 Cargo.toml
[features]
tests-mock = ["dep:tests-mock-s3", "dep:tests-mock-vault"]

[dev-dependencies]
tests-mock-s3 = { path = "D:/tests-mock/crates/tests-mock-s3", optional = true }
tests-mock-vault = { path = "D:/tests-mock/crates/tests-mock-vault", optional = true }
```

```bash
# 跑测试时打开 feature
cargo test --features tests-mock
```

### 方式 3：脚本驱动（端到端）

```powershell
# Windows PowerShell
& D:/tests-mock/scripts/init_mock_env.ps1
& D:/tests-mock/scripts/seed_fixtures.ps1
& D:/tests-mock/scripts/run_smoke_test.ps1
& D:/tests-mock/scripts/cleanup_mock_env.ps1
```

```bash
# Linux / WSL
python3 D:/tests-mock/scripts/init_mock_env.py
python3 D:/tests-mock/scripts/seed_fixtures.py
python3 D:/tests-mock/scripts/run_smoke_test.py
python3 D:/tests-mock/scripts/cleanup_mock_env.py
```

---

## 当前阶段：v0.3 Phase 2 in-process 实装

- 4 个 trait mock backend (`s3` / `vault` / `git` / `ai`) 的 25 个 trait method 全部实装（in-process `Arc<Mutex<HashMap<...>>>` / BTreeMap，无 `unimplemented!()` 残留）
- `tests-mock-core` 共享底盘新增 `port: u16` + `pid: Option<u32>` 字段，并提供 `init_mock_env` / `cleanup_mock_env` 顶层 helper（脚本侧友好别名）
- run_smoke_test 报告行数：4 trait backend × 5 method = 20 + 1 core meta = 21 entries
- 22 个新 unit test（覆盖 head_bucket / put_object / vault get-set-delete / git init+upload / ai complete+stream / core lifecycle helper）

### 验证

```bash
cd D:/tests-mock
cargo check --workspace
cargo test --workspace         # 30 旧 + 22 新 = 52 tests / 0 fail
cargo clippy --workspace --all-targets -- -D warnings
```

预期：0 warning / 0 error / 0 clippy 警告（trait 实装 + 单测全 pass）

---

## 修订历史

| 版本 | 日期 (JST) | 修订人 | 摘要 | 审批 |
| --- | --- | --- | --- | --- |
| v0.1 | 2026-08-31 16:14 | Ulysses（一人公司 12 角色 per DEC-008）— Mavis 接手 | 初版：workspace 骨架 + 5 trait stub + 7 段设计书 | 架构师 (Mavis 接手 agent per DEC-008) |
| v0.2 | 2026-08-31 16:43 | Ulysses（一人公司 12 角色 per DEC-008）— Mavis 接手 | 5 脚本双版本（PS + Python）：init/seed/smoke/stress/cleanup + 3 JSON fixture（user_creds/repo_metadata/ai_response_cache）+ loader helper（`load_user_creds`/`load_repo_metadata`/`load_ai_response_cache`，20 单测全 pass） | 架构师 (Mavis 接手 agent per DEC-008) |
| v0.3 | 2026-08-31 17:50 | Ulysses（一人公司 12 角色 per DEC-008）— Mavis 接手 | Phase 2 in-process 实装：4 trait mock backend × 5 method = 25 method 全实装（`Arc<Mutex<HashMap<...>>>` / BTreeMap，无 `unimplemented!()` 残留）+ 22 新 unit test（52 total / 0 fail / 0 clippy warn）+ core `init_mock_env` / `cleanup_mock_env` helper + port/pid 字段 + run_smoke_test 数字修正（4 trait + 1 core meta = 21 entries） | 架构师 (Mavis 接手 agent per DEC-008) |
