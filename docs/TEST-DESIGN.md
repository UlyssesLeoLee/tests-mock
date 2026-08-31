# tests-mock 测试设计书

> 跨项目测试 mock 脚手架 — 7 段设计书
>
> 版本：v0.1  ·  日期：2026-08-31 JST  ·  修订人：Ulysses（一人公司 12 角色 per DEC-008）— Mavis 接手
> 审批：架构师 (Mavis 接手 agent per DEC-008)

---

## §0 目的

### 0.1 仓定位

`tests-mock` 是与 `GitGit` / `RustGameServer` / `Physis` / `Star` **并列** 的独立跨项目测试脚手架，专门为**测试场景**提供：

- **5 个 mock backend**（s3 / vault / git / ai / core）
- **3 份 JSON fixture**（user_creds / repo_metadata / ai_response_cache）
- **5 套脚本**（init / seed / smoke / stress / cleanup，双版本 PowerShell + Python）
- **统一 trait 抽象**（下游 Rust 项目可 `cargo test --features tests-mock` 引用）

### 0.2 设计目标

| 目标 | 度量 | 优先级 |
| --- | --- | --- |
| 跨项目一致性 | 5 mock backend trait 形状对齐；任何下游项目集成 ≤ 1 h | P0 |
| 行为可预测 | 100% 幂等 + 失败回滚 | P0 |
| 零业务耦合 | docs-only stub 阶段 trait method 全 `unimplemented!()` | P0 |
| 易于扩展 | 新 mock backend 加 1 crate 即可，不动既有 | P1 |
| 可压测 | 100 并发 × 1000 次 P99 latency < 100 ms（in-process 模式） | P1 |
| 端口兼容 | 5 脚本双版本行为完全一致（PS + Python） | P1 |

### 0.3 不在范围

- ❌ 不实装 5 mock backend 行为（docs/stub 阶段留 Phase 2 子代理 B）
- ❌ 不生产化（无 release 流程、无 crates.io 发布）
- ❌ 不推 origin（per R-05 维持，远程 `https://github.com/UlyssesLeoLee/tests-mock.git` 留 V1+）
- ❌ 不替下游项目做集成测试（仅暴露 trait + 脚本）

---

## §1 范围

### 1.1 5 mock backend 范围

| Crate | 模拟目标 | 5 个 mock 行为 | 失败场景覆盖 |
| --- | --- | --- | --- |
| `tests-mock-s3` | minIO / AWS S3 | `head_bucket` / `put_object` / `get_object` / `list_objects` / `delete_object` | bucket 不存在 / key 不存在 / body 越界 / prefix 越界 / 中途取消 |
| `tests-mock-vault` | HashiCorp Vault / VaultCache | `get` / `set` / `delete` / `list` / `rotate` | key 不存在 / 重复 rotate / 锁竞争 / 中途取消 |
| `tests-mock-git` | Git server / gitea / daemon | `init_bare` / `receive_pack` / `upload_pack` / `get_refs` / `list_refs` | 重复 init / 非法 pkt-line / 缺失 want / glob 不匹配 / 大包体 |
| `tests-mock-ai` | OpenAI / Anthropic / DeepSeek | `complete` / `embed` / `stream_token` / `cancel` / `usage_stats` | 超时 / token 截断 / 重复 cancel / 跨模型聚合 / 并发限流 |
| `tests-mock-core` | 共享底盘 | `MockError` / `MockConfig` / `MockLifecycle` | serde 错误 / mode 转换错误 / start 失败 |

### 1.2 3 fixture 范围

| Fixture | schema 版本 | 字段 | 用途 |
| --- | --- | --- | --- |
| `user_creds.json` | v0.1 | `version` / `users[]` / 每个 user 含 `name` + `vault_keys[]` | vault 注入 / auth 链 / 跨用户隔离 |
| `repo_metadata.json` | v0.1 | `version` / `repos[]` / 每个 repo 含 `id` / `title` / `labels[]` / `priority` / `status` | git mock 仓库元数据 / issue 追踪链路 |
| `ai_response_cache.json` | v0.1 | `version` / `responses[]` / 每个 response 含 `prompt_hash` / `model` / `completion` / `tokens` / `latency_ms` | AI mock 缓存命中 / 离线 deterministic / 延迟模拟 |

### 1.3 5 脚本范围

| 脚本 | PS + Python | 行为 |
| --- | --- | --- |
| `init_mock_env` | ✓ | 起本地 mock 环境（docker compose or in-process）/ 写 state 文件 / 健康检查 / 失败回滚 |
| `seed_fixtures` | ✓ | 加载 3 份 JSON 到 mock backend |
| `run_smoke_test` | ✓ | 端到端跑 5 mock backend 核心路径 / 输出 JSON 报告 |
| `stress_concurrency` | ✓ | 100 并发 × 1000 次压测 / 输出 P50 / P95 / P99 latency + error rate |
| `cleanup_mock_env` | ✓ | 关停 backend / 清临时文件 / 输出 summary |

### 1.4 Phase 边界

| Phase | 内容 | 子代理 |
| --- | --- | --- |
| Phase 1（当前） | workspace 骨架 + trait stub + 7 段设计书 | A (Mavis 接手) |
| Phase 2 | 5 mock backend in-process 实装 | B |
| Phase 2 | 3 JSON fixture 数据 + loader helper | C |
| Phase 2 | 5 脚本双版本实装 | B |
| V1+ | docker compose 模式 / CI 集成 / GitHub 仓创建 | D |

---

## §2 设计

### 2.1 5 mock backend 交互图

```
                       ┌──────────────────────┐
                       │  tests-mock-core     │
                       │  error / config /    │
                       │  lifecycle           │
                       └──────────┬───────────┘
                                  │ 共享
        ┌──────────────┬──────────┼──────────┬──────────────┐
        ▼              ▼          ▼          ▼              ▼
  ┌──────────┐  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
  │ tests-   │  │ tests-   │ │ tests-   │ │ tests-   │ │ tests-   │
  │ mock-s3  │  │ mock-    │ │ mock-git │ │ mock-ai  │ │ mock-    │
  │          │  │ vault    │ │          │ │          │ │ fixtures │
  │ 5 行为   │  │ 5 行为   │ │ 5 行为   │ │ 5 行为   │ │ 3 schema │
  └────┬─────┘  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘
       │             │            │            │            │
       └─────────────┴─────┬──────┴────────────┴────────────┘
                           │
                  ┌────────▼────────┐
                  │  scripts/        │
                  │  init / seed /   │
                  │  smoke / stress  │
                  │  / cleanup       │
                  │  (PS + Python)   │
                  └────────┬────────┘
                           │
                  ┌────────▼────────┐
                  │  下游项目        │
                  │  GitGit / RGS /  │
                  │  Physis / Star   │
                  │  feature:        │
                  │  tests-mock      │
                  └─────────────────┘
```

### 2.2 数据流（in-process 模式）

```
init_mock_env
  │
  ├─→ 启动 5 个 mock backend (in-process HashMap / BTreeMap)
  ├─→ 写 /tmp/tests-mock-state.json
  └─→ 健康检查 ping 5 backend

seed_fixtures
  │
  ├─→ 读 fixtures/user_creds.json
  ├─→ 注入 vault (vault.set)
  ├─→ 读 fixtures/repo_metadata.json
  ├─→ 注入 git (init_bare)
  └─→ 读 fixtures/ai_response_cache.json
       └─→ 注入 ai 内存 cache

run_smoke_test
  │
  ├─→ s3: head_bucket → put_object → get_object → list_objects → delete_object
  ├─→ vault: get → set → list → rotate
  ├─→ git: init_bare → receive_pack → get_refs → list_refs
  ├─→ ai: complete → stream_token → cancel → usage_stats
  └─→ 写 /tmp/tests-mock-smoke-report.json

stress_concurrency
  │
  ├─→ 100 并发 × 1000 次 vault get / set / ai stream_token / s3 put_object
  └─→ 输出 P50 / P95 / P99 latency + error rate

cleanup_mock_env
  │
  ├─→ 关停 5 backend (优雅)
  ├─→ 清 /tmp/tests-mock-state.json
  └─→ 输出 summary
```

### 2.3 模式选择

| 模式 | 触发条件 | 优 | 劣 |
| --- | --- | --- | --- |
| `InProcess` | 默认（无 docker daemon） | 零外部依赖 / 启动快 / 跨平台一致 | 不能模拟网络抖动 / 不可分布式 |
| `Docker` | `MOCK_MODE=docker` 环境变量 | 真实协议栈 / 可观测 / 可压真实延迟 | 需 docker daemon / 慢 |

---

## §3 用例

> 每个 mock backend 列 5-10 个测试场景，含失败 / 并发 / 降级。

### 3.1 tests-mock-s3（10 场景）

| # | 场景 | 期望 |
| --- | --- | --- |
| S3-01 | head_bucket 已存在 | `Ok(())` |
| S3-02 | head_bucket 不存在 | `Err(NotFound)` |
| S3-03 | put_object 后 get_object 拿到原 body | bytes 完全一致 |
| S3-04 | put_object 覆盖语义（同名 key 二次写入） | 后写胜出 |
| S3-05 | get_object 不存在的 key | `Err(NotFound)` |
| S3-06 | list_objects 按 prefix 过滤 | 仅返回匹配 key |
| S3-07 | delete_object 后再 get_object | `Err(NotFound)` |
| S3-08 | 大 body（1 MiB）put + get | 字节完全一致 |
| S3-09 | 并发 100 × 1000 次 put_object | error rate < 0.1% / 无数据竞争 |
| S3-10 | list_objects 跨 10000 key 性能 | P99 < 50 ms（in-process） |

### 3.2 tests-mock-vault（10 场景）

| # | 场景 | 期望 |
| --- | --- | --- |
| VA-01 | get 已存在 key | `Ok(Some(value))` |
| VA-02 | get 不存在 key | `Ok(None)`（非 Err） |
| VA-03 | set 覆盖语义 | 后写胜出 |
| VA-04 | delete 后 get | `Ok(None)` |
| VA-05 | list 返回全部 key | 顺序无关 / 集合一致 |
| VA-06 | rotate 后旧 value grace period 仍可读 | 旧 value 在 grace 内 `Ok(Some(old))` |
| VA-07 | rotate 二次（旧 value 过期后） | 旧 value `Ok(None)` / 新 value `Ok(Some(new))` |
| VA-08 | 并发 100 × 1000 次 set 同一 key | 无丢失（last-write-wins） |
| VA-09 | lock contention 模拟 | 锁等待 P99 < 50 ms |
| VA-10 | rotate 失败回滚 | secret 不变 / 返回 `Err(Backend)` |

### 3.3 tests-mock-git（10 场景）

| # | 场景 | 期望 |
| --- | --- | --- |
| GT-01 | init_bare 新仓库 | `Ok(())` |
| GT-02 | init_bare 重复 | `Err(AlreadyExists)` |
| GT-03 | receive_pack 合法 pkt-line | `Ok(())` / ref 更新 |
| GT-04 | receive_pack 非法 pkt-line | `Err(InvalidInput)` / ref 不变 |
| GT-05 | upload_pack with wants | 返回 pack bytes |
| GT-06 | upload_pack 缺失 want | `Err(InvalidInput)` |
| GT-07 | get_refs 列出全部 ref | 与 init 时一致 |
| GT-08 | list_refs glob 过滤 | 仅匹配 ref |
| GT-09 | 并发 100 × 1000 receive_pack | 无丢包 / 顺序保留 |
| GT-10 | 大 pack（1 MiB） | 字节一致 / P99 < 200 ms |

### 3.4 tests-mock-ai（10 场景）

| # | 场景 | 期望 |
| --- | --- | --- |
| AI-01 | complete 短 prompt | 返回确定性 mock completion（命中 cache） |
| AI-02 | complete 未知 prompt | 返回默认 mock + warning |
| AI-03 | embed 文本 | 返回 1536 维 Vec<f32> / 模长 ≈ 1 |
| AI-04 | stream_token 启动 | 返回 TokenStream / 首 token < 100 ms |
| AI-05 | stream_token 取消（mid-stream） | `Err(Cancelled)` / partial tokens 已落 |
| AI-06 | cancel 不存在的 request_id | `Err(NotFound)` |
| AI-07 | usage_stats since_unix_ms 聚合 | 数字与逐次请求一致 |
| AI-08 | usage_stats 跨模型聚合 | by_model HashMap 一致 |
| AI-09 | 并发 100 × 1000 stream_token | 100% 成功率 / token 数不丢 |
| AI-10 | 超时（prompt 包含 "timeout" 关键字） | `Err(Timeout)` / 100 ms 内 |

### 3.5 tests-mock-core（5 场景）

| # | 场景 | 期望 |
| --- | --- | --- |
| CO-01 | MockError Display 全 variant | 包含 kind 字符串 |
| CO-02 | MockError 满足 `std::error::Error` | trait bound 通过 |
| CO-03 | MockConfig serde roundtrip | JSON → struct → JSON 一致 |
| CO-04 | MockConfig Default | InProcess 模式 + 默认路径 |
| CO-05 | MockLifecycle start / health / stop 编排 | 顺序调用通过 |

**总计：45 个测试场景**（5 backend × 9 平均 = 45）

---

## §4 数据

### 4.1 Fixture schema 严格定义

#### 4.1.1 `user_creds.json`

```json
{
  "$schema": "https://json-schema.org/draft-07/schema#",
  "version": "v0.1",
  "users": [
    {
      "name": "alice",
      "vault_keys": ["openai:key", "github:pat", "gitee:pat"]
    },
    {
      "name": "bob",
      "vault_keys": ["anthropic:key", "gitlab:pat"]
    }
  ],
  "test_key": "sk-mock-test-only-not-real"
}
```

字段约束：

| 字段 | 类型 | 必填 | 约束 |
| --- | --- | --- | --- |
| `version` | string | ✓ | 固定 `v0.1` |
| `users` | array | ✓ | 至少 1 项 |
| `users[].name` | string | ✓ | 非空 / 唯一 |
| `users[].vault_keys` | array<string> | ✓ | 至少 1 项 |
| `test_key` | string | ✓ | 必须以 `sk-mock-` 前缀（防止误用真 key） |

#### 4.1.2 `repo_metadata.json`

```json
{
  "$schema": "https://json-schema.org/draft-07/schema#",
  "version": "v0.1",
  "repos": [
    {
      "id": "STAR-1024",
      "title": "Mock issue STAR-1024",
      "labels": ["mock"],
      "priority": "MEDIUM",
      "status": "OPEN"
    },
    {
      "id": "STAR-1025",
      "title": "Mock issue STAR-1025",
      "labels": ["mock", "ai-ide-compat"],
      "priority": "HIGH",
      "status": "IN_PROGRESS"
    }
  ]
}
```

字段约束：

| 字段 | 类型 | 必填 | 约束 |
| --- | --- | --- | --- |
| `version` | string | ✓ | 固定 `v0.1` |
| `repos` | array | ✓ | 至少 1 项 |
| `repos[].id` | string | ✓ | 匹配 `^[A-Z]+-\d+$` |
| `repos[].title` | string | ✓ | 非空 |
| `repos[].labels` | array<string> | ✓ | 可空数组 |
| `repos[].priority` | enum | ✓ | `LOW` / `MEDIUM` / `HIGH` / `CRITICAL` |
| `repos[].status` | enum | ✓ | `OPEN` / `IN_PROGRESS` / `CLOSED` |

#### 4.1.3 `ai_response_cache.json`

```json
{
  "$schema": "https://json-schema.org/draft-07/schema#",
  "version": "v0.1",
  "responses": [
    {
      "prompt_hash": "abc123",
      "model": "gpt-4o-mini",
      "completion": "Mock commit message for testing",
      "tokens": 42,
      "latency_ms": 235
    },
    {
      "prompt_hash": "def456",
      "model": "claude-3-5-haiku-latest",
      "completion": "Mock review comment",
      "tokens": 128,
      "latency_ms": 412
    }
  ]
}
```

字段约束：

| 字段 | 类型 | 必填 | 约束 |
| --- | --- | --- | --- |
| `version` | string | ✓ | 固定 `v0.1` |
| `responses` | array | ✓ | 至少 1 项 |
| `responses[].prompt_hash` | string | ✓ | 匹配 `^[a-f0-9]{6,64}$` |
| `responses[].model` | string | ✓ | 已知模型枚举 |
| `responses[].completion` | string | ✓ | 非空 |
| `responses[].tokens` | integer | ✓ | ≥ 1 |
| `responses[].latency_ms` | integer | ✓ | 0 < x < 60000 |

### 4.2 加载方式

```rust
// crates/tests-mock-fixtures/src/lib.rs (Phase 2 子代理 C 实装)
pub fn load_user_creds() -> MockResult<UserCreds>;
pub fn load_repo_metadata() -> MockResult<RepoMetadata>;
pub fn load_ai_response_cache() -> MockResult<AiResponseCache>;
```

loader 行为：

1. 读 `fixtures/<name>.json`（相对 CARGO_MANIFEST_DIR）
2. `serde_json::from_str` parse
3. 校验 `version` 字段 = `v0.1`
4. 返回 typed struct

### 4.3 注入 mock backend

```rust
// scripts/seed_fixtures.rs (Phase 2 子代理 B 实装)
let creds = tests_mock_fixtures::load_user_creds()?;
for user in creds.users {
    for (i, key) in user.vault_keys.iter().enumerate() {
        vault.set(key, &format!("mock-value-{user}-{i}")).await?;
    }
}
```

---

## §5 验收

### 5.1 tests-mock-s3 AC

- [ ] AC-S3-1：5 个 mock 行为 trait 方法均编译通过（docs-only stub）
- [ ] AC-S3-2：trait method body 含 `unimplemented!()`，运行即 panic
- [ ] AC-S3-3：模块 `use tests_mock_core::error::MockError` 引用统一错误
- [ ] AC-S3-4：`S3Error` 别名 = `MockError`
- [ ] AC-S3-5：`InMemoryS3` stub struct + `new()` panic 占位

### 5.2 tests-mock-vault AC

- [ ] AC-VA-1 ~ AC-VA-5（同上模式）
- [ ] AC-VA-6：`get` 返回 `MockResult<Option<String>>` 区分"不存在"与"错误"

### 5.3 tests-mock-git AC

- [ ] AC-GT-1 ~ AC-GT-5（同上模式）
- [ ] AC-GT-6：`receive_pack` 接受 `&[u8]`（pkt-line 字节流）

### 5.4 tests-mock-ai AC

- [ ] AC-AI-1 ~ AC-AI-5（同上模式）
- [ ] AC-AI-6：`usage_stats` 用 `i64 since_unix_ms` 替代 chrono DateTime
- [ ] AC-AI-7：`TokenStream` struct 可序列化
- [ ] AC-AI-8：`UsageReport` struct 可序列化

### 5.5 tests-mock-core AC

- [ ] AC-CO-1：`MockError` Display 8 个 variant 全覆盖
- [ ] AC-CO-2：`MockError: std::error::Error`
- [ ] AC-CO-3：`MockConfig` serde roundtrip
- [ ] AC-CO-4：`MockConfig::default()` 合法

### 5.6 tests-mock-fixtures AC（commit 2 阶段补）

- [ ] AC-FX-1：3 份 JSON 在 `fixtures/` 目录
- [ ] AC-FX-2：3 个 loader 函数暴露 `load_<name>()`
- [ ] AC-FX-3：每个 loader 5-10 单测（serde 解析 + 字段访问）
- [ ] AC-FX-4：version 字段校验

### 5.7 scripts AC（commit 2 阶段补）

- [ ] AC-SC-1：5 脚本 × 2 版本 = 10 文件齐全
- [ ] AC-SC-2：shebang + UTF-8 BOM（PS 兼容）
- [ ] AC-SC-3：幂等（重复执行不破坏）
- [ ] AC-SC-4：cleanup 即使 smoke 中途失败也清理干净
- [ ] AC-SC-5：0 外部依赖（pwsh 内置 / Python stdlib + json + pathlib）

### 5.8 总体 AC

- [ ] AC-OV-1：`cargo check --workspace` 0 warning / 0 error
- [ ] AC-OV-2：`cargo test --workspace` 全 pass
- [ ] AC-OV-3：commit author = `Ulysses <ulysses@mavis.local>`
- [ ] AC-OV-4：不推 origin（per R-05）
- [ ] AC-OV-5：0 unsafe / 0 业务逻辑（仅 trait + 错误类型 + 测试）

---

## §6 守门

### 6.1 硬约束（per user_profile 强证据）

| 约束 | 来源 | 实现 |
| --- | --- | --- |
| 0 unsafe | 自定 / 2026-08-21 团队基线 | 全 crate `#![forbid(unsafe_code)]` |
| 0 新外部依赖 | 自定 | workspace 仅有 `serde` / `serde_json` / `tokio` |
| 不污染下游 | 自定 | 5 mock crate 不 import 下游项目 / 路径硬编码到 `D:/tests-mock/` |
| 不推 origin | R-05 / 2026-08-27 11:09 JST | `git init` 后不 `git remote add` / 不 `git push` |
| PowerShell only | platform: win32 | 所有脚本用 `;` 替 `&&` / `Get-ChildItem` 替 `ls -la` / `Select-String` 替 `grep` |
| 禁环境变量打印 | 2026-08-27 11:06 JST | 脚本不 `Get-ChildItem env:` / 不 `echo $VAR` / 不 `cat .env` |
| 代签规则 | 2026-08-27 19:39/20:56/21:59 JST | author = Ulysses；审批 = 架构师(Mavis 接手) |
| 禁回溯叙事 | 2026-08-26 强证据 | 不写"per X 历史形态" / "原本是" / 引用 BAS 必 `git log -p --follow` |

### 6.2 安全网

```rust
// 每个 mock crate 顶部加
#![forbid(unsafe_code)]
#![deny(missing_docs)]
```

### 6.3 已知缺口

> 缺标比错标安全（per 2026-08-26 强证据）

- ⚠️ Phase 1 未实装 5 mock backend 行为（trait method `unimplemented!()` 命中即 panic）
- ⚠️ Phase 1 未生成 3 份 JSON fixture 数据（commit 2 阶段补）
- ⚠️ Phase 1 未实装 5 脚本行为（commit 2 阶段补）
- ⚠️ docker compose 模式未实装（V1+）
- ⚠️ GitHub 仓未创建（V1+）

---

## §7 签字栏

> 5 角色 per AGENTS.md 模板 / 5 域独立真实身份 per 2026-08-21 JST

| 角色 | 姓名 | 签字 | 日期 (JST) | 备注 |
| --- | --- | --- | --- | --- |
| 架构 | Ulysses（一人公司 12 角色 per DEC-008）— Mavis 接手 | ✅ 代签 | 2026-08-31 16:14 | 框架 + 7 段设计书主笔 |
| SRE Lead | Ulysses（一人公司 12 角色 per DEC-008）— Mavis 接手 | ⏳ DDD Review | — | 部署 / 监控待 DDD Review |
| 平台 | Ulysses（一人公司 12 角色 per DEC-008）— Mavis 接手 | ⏳ DDD Review | — | 跨项目集成待 DDD Review |
| 评审 | Ulysses（一人公司 12 角色 per DEC-008）— Mavis 接手 | ⏳ DDD Review | — | 用例评审待 DDD Review |
| PM | Ulysses（一人公司 12 角色 per DEC-008）— Mavis 接手 | ⏳ DDD Review | — | 进度 / WBS 待 DDD Review |

> 5 域独立真实身份 per 2026-08-21 JST：DDD Review 阶段可补 SRE Lead / 平台 / 评审 / PM 4 域 Lead 真实身份（拒绝"架构师兼任" / "SRE 兼任" 兼任方案）。
