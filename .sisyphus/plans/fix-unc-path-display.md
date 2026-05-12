# 修复 Windows UNC 路径显示问题

## TL;DR

> **Quick Summary**: Windows 上 `canonicalize()` 返回 UNC 路径格式（`\\?\E:\...`），对用户不友好。使用 `dunce` crate 转换为普通路径。
>
> **Deliverables**:
> - Cargo.toml 添加 dunce 依赖
> - app_commands.rs 使用 dunce::canonicalize()
>
> **Estimated Effort**: Quick（约 5 分钟）
> **Parallel Execution**: NO - 单文件顺序修改
> **Critical Path**: Cargo.toml → app_commands.rs

---

## Context

### Original Request
用户反馈仓库列表中显示的路径格式为 UNC 格式（如 `\\?\E:\Git_Repositories\AegisOTA`），不够友好。

### Root Cause
Rust 的 `std::path::Path::canonicalize()` 在 Windows 上会返回扩展长度路径格式（Extended-Length Path），这是 Windows API 的特性，用于支持超过 260 字符的路径。但这个格式对用户来说不直观。

---

## Work Objectives

### Core Objective
将 UNC 路径转换为用户友好的普通路径格式（如 `E:\Git_Repositories\AegisOTA`）。

### Concrete Deliverables
- Cargo.toml 中添加 `dunce = "1"` 依赖
- app_commands.rs 中使用 `dunce::canonicalize()` 替代 `std::path::Path::canonicalize()`

### Definition of Done
- [x] Cargo.toml 包含 dunce 依赖
- [x] app_commands.rs 使用 dunce
- [x] cargo build 成功
- [x] 实际测试：添加仓库后显示的路径为普通格式

### Must Have
- 路径显示为普通格式，无 UNC 前缀

### Must NOT Have
- 不改变其他逻辑（canonicalize 的目的仍然是验证路径有效性）

---

## Verification Strategy

### Test Decision
- **Infrastructure exists**: YES（cargo test）
- **Automated tests**: Tests-after（修改后验证）
- **Framework**: cargo test

### QA Policy
- 使用 cargo build 验证编译成功
- 手动测试：添加仓库后检查显示的路径格式

---

## TODOs

- [x] 1. 添加 dunce crate 到 Cargo.toml

  **What to do**:
  - 在 Cargo.toml 的 [dependencies] 部分添加 `dunce = "1"`

  **Must NOT do**:
  - 不修改其他依赖

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []（简单文本编辑）

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Task 2
  - **Blocked By**: None

  **References**:
  - `src-tauri/Cargo.toml` - 需要添加依赖的位置

  **Acceptance Criteria**:
  - [ ] Cargo.toml 包含 `dunce = "1"`

  **QA Scenarios**:
  ```
  Scenario: 验证 Cargo.toml 格式正确
    Tool: Bash
    Steps:
      1. cat src-tauri/Cargo.toml
      2. 检查输出包含 "dunce = "1"
    Expected Result: dunce 依赖存在
    Evidence: .sisyphus/evidence/task-1-cargo.toml
  ```

  **Commit**: YES（与 Task 2 一起）
  - Message: `fix: use dunce to convert UNC paths to friendly format`
  - Files: `src-tauri/Cargo.toml, src-tauri/src/app_commands.rs`

---

- [x] 2. 修改 add_repository 使用 dunce::canonicalize()

  **What to do**:
  - 在 app_commands.rs 顶部添加 `use dunce;`
  - 将第 236 行的 `repo_path.canonicalize()` 改为 `dunce::canonicalize(&repo_path)`
  - 注意：dunce::canonicalize 接受引用参数

  **Must NOT do**:
  - 不改变其他逻辑
  - 不修改错误处理方式

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []（简单代码修改）

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Verification
  - **Blocked By**: Task 1

  **References**:
  - `src-tauri/src/app_commands.rs:236` - 需要修改的代码位置

  **Acceptance Criteria**:
  - [ ] cargo build 成功
  - [ ] 路径显示为普通格式

  **QA Scenarios**:
  ```
  Scenario: 验证编译成功
    Tool: Bash
    Steps:
      1. cargo build --manifest-path src-tauri/Cargo.toml
    Expected Result: Build 成功，无错误
    Evidence: .sisyphus/evidence/task-2-build.txt

  Scenario: 验证路径格式
    Tool: Bash (cargo tauri dev)
    Steps:
      1. 运行 cargo tauri dev
      2. 在设置页面添加一个仓库
      3. 检查显示的路径格式
    Expected Result: 路径显示为普通格式（如 E:\...），无 \\?\ 前缀
    Evidence: .sisyphus/evidence/task-2-path-format.png
  ```

  **Commit**: YES（与 Task 1 一起）

---

## Final Verification Wave

- [x] F1. 编译验证
  - cargo build 成功
  - cargo test 成功

- [x] F2. 手动测试
  - 运行 cargo tauri dev
  - 添加仓库，检查路径显示格式

---

## Commit Strategy

- **Single Commit**: `fix: use dunce to convert UNC paths to friendly format on Windows`
- **Files**: `src-tauri/Cargo.toml, src-tauri/src/app_commands.rs`