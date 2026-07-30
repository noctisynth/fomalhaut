# Fomalhaut 技术设计

## 1. 项目概述

Fomalhaut（北落师门）是一个基于
[greetd](https://git.sr.ht/~kennylevinsen/greetd) 的 Web 渲染登录界面
（greeter）。

Fomalhaut 不负责直接调用 PAM，也不重新实现用户会话管理。认证和会话启动仍由
greetd 完成。Fomalhaut 通过 greetd IPC 驱动认证流程，在本地 WebView 中呈现界面，
并允许管理员或主题作者自行提供 HTML、CSS 和 JavaScript 前端。

本项目是独立实现。`tuigreet` 仅作为 greetd IPC 行为、会话发现和测试方法的参考，
不作为 Fomalhaut 的源码基础。

## 2. 设计目标

### 2.1 核心目标

- 将 greetd IPC 和认证状态机封装为不依赖具体 UI 的 Rust core。
- 使用本地 WebView 渲染登录界面。
- 不固化前端框架、构建工具或视觉设计。
- 允许用户直接配置主题目录，或自行编写完整前端。
- 为前端提供稳定、版本化且最小化的消息协议。
- 在不信任前端内容的前提下限制其系统权限。
- 正确支持 PAM 的多轮、任意类型认证提示，而非仅支持“用户名 + 密码”。
- 让 core、会话发现和协议转换能够在无图形环境中完成自动化测试。

### 2.2 次要目标

- 支持发现 X11 和 Wayland desktop session。
- 支持保存上次用户和上次会话等非敏感偏好。
- 提供关机、重启等经过配置和授权的系统操作。
- 提供便于主题开发的独立预览或演示模式。
- 提供一个极简示例主题，用于协议演示和集成测试。

### 2.3 非目标

- 不实现 PAM 或替代 greetd。
- 不提供远程登录网页或监听局域网的登录服务。
- 不允许前端直接连接 greetd socket。
- 不允许前端提交任意可执行文件、命令行、环境变量或文件路径。
- 不承诺 JavaScript/WebView 内存中的密码能够像 Rust 缓冲区一样可靠清零。
- 初期不实现完整的窗口管理器或 Wayland compositor。
- 初期不提供通用浏览器能力，例如任意导航、下载、扩展或开发者工具。

## 3. 系统边界

```text
greetd
  │
  │ Unix socket（GREETD_SOCK）
  ▼
fomalhaut-core
  │
  │ 类型化 Rust API / 认证事件
  ▼
Fomalhaut host
  ├── session discovery
  ├── configuration
  ├── policy enforcement
  ├── WebView lifecycle
  └── versioned frontend bridge
          │
          │ 受限的双向消息
          ▼
用户提供的 Web 前端
  HTML / CSS / JavaScript / 静态资源
```

只有 Rust core 可以访问 greetd socket。Web 前端不能获得 socket、原始文件系统访问能力
或任意进程执行能力。

Fomalhaut 是本机登录界面，不是 Web 服务器。正式运行时应通过 WebView 自定义资源协议
或等价的进程内资源加载机制提供前端文件，而不是开放 TCP 端口。

## 4. 建议的 workspace 结构

Fomalhaut 使用虚拟 Cargo workspace，统一采用以下项目基线：

- 许可证：GNU Affero General Public License v3.0（SPDX：`AGPL-3.0-only`）。
- Rust Edition：2024。
- Rust 工具链：跟随最新 stable 滚动更新，不声明或维护固定 MSRV。
- Cargo feature resolver：版本 3。
- Workspace 成员：通过 `crates/*` 自动包含 `crates/` 下的所有 crate。

根 `Cargo.toml` 只包含 workspace 定义、共享 package 元数据和 workspace 级 lint，
不包含根 package。由于 Cargo CLI 不能创建虚拟 workspace，项目初始化时允许手工创建一次
根 manifest；各成员 crate 仍必须使用 `cargo new` 创建，依赖仍必须通过 `cargo add` 和
`cargo remove` 管理。

仓库通过 `rust-toolchain.toml` 选择 `stable` channel，并安装 rustfmt 和 Clippy 组件。
本地开发和 CI 都使用该滚动 channel；不保留旧工具链兼容性 job。stable 更新导致的编译、
lint 或行为变化应作为正常维护工作及时修复，而不是通过固定旧版本规避。

依赖同样采用滚动最新策略：

- 新增依赖时，`cargo add` 默认选择执行时可用的最新稳定版本。
- 不使用 `*` 等无上界约束；manifest 保存 Cargo 生成的语义化版本要求。
- 提交 `Cargo.lock`，保证任一提交可以复现其已验证的依赖集合。
- 持续更新 lockfile，并主动升级不在当前版本要求范围内的新主版本。
- 依赖升级必须通过格式、Clippy、测试和文档构建；如果升级需要改变技术方案，仍须先更新
  本文和 `TODO.md`。

```text
fomalhaut/
├── Cargo.toml
├── crates/
│   ├── fomalhaut-core/
│   │   └── src/
│   │       ├── client.rs
│   │       ├── error.rs
│   │       ├── event.rs
│   │       ├── secret.rs
│   │       ├── state.rs
│   │       └── transport.rs
│   ├── fomalhaut-session/
│   │   └── src/
│   │       ├── desktop_entry.rs
│   │       └── discovery.rs
│   ├── fomalhaut-web/
│   │   └── src/
│   │       ├── assets.rs
│   │       ├── bridge.rs
│   │       ├── protocol.rs
│   │       └── webview.rs
│   └── fomalhaut/
│       └── src/
│           ├── config.rs
│           ├── main.rs
│           └── policy.rs
├── protocol/
│   └── v1.schema.json
├── examples/
│   └── minimal-theme/
└── docs/
    └── DESIGN.md
```

各 crate 的职责如下：

### 4.1 `fomalhaut-core`

- 连接 greetd Unix socket。
- 序列化和反序列化 greetd IPC。
- 实现严格的认证状态机。
- 把 greetd 的响应转换为 UI 无关的事件。
- 管理 PAM 回答等敏感数据的生命周期。
- 拒绝非法、重复或过期操作。
- 提供可替换 transport，便于使用 stub 测试。

core 不负责：

- 用户和 session 列表的视觉呈现。
- WebView 或 JavaScript bridge。
- 从不可信前端解析命令行。
- 保存主题偏好。
- 执行任意电源命令。

### 4.2 `fomalhaut-session`

- 从受信任的配置目录发现 desktop entry。
- 区分 X11 和 Wayland session。
- 解析显示名称、可执行命令和必要的元数据。
- 将文件系统内容转换为不透明的 `SessionId`。
- 根据策略过滤隐藏、无效或被禁止的 session。

前端只能选择 `SessionId`。实际命令始终由 Rust host 根据已发现的 session 生成。

### 4.3 `fomalhaut-web`

- 创建并管理本地 WebView。
- 从主题目录加载静态资源。
- 实现自定义资源 scheme，例如 `fomalhaut://theme/`。
- 在 Rust 类型和版本化 JSON 消息之间转换。
- 限制导航、新窗口、下载、网络、剪贴板和开发者工具。
- 在页面刷新、崩溃或连接丢失时通知 host。

该 crate 不包含正式产品主题。仓库中的 minimal theme 仅用于示例、开发和测试。

### 4.4 `fomalhaut`

- 读取和验证系统配置。
- 组合 core、session discovery 和 Web host。
- 维护面向前端的公开状态快照。
- 执行 session ID 到可信 `SessionCommand` 的映射。
- 应用电源操作、主题路径和持久化策略。
- 管理进程退出码、日志和恢复页面。

### 4.5 Monorepo 版本与发布

Fomalhaut 使用 Semifold（CLI：`smif`）管理 monorepo changeset、独立包版本和发布：

- Semifold 使用 Rust workspace resolver。
- changeset 存放在仓库根目录的 `.changes/`。
- 每个 crate 在自己的 `Cargo.toml` 中保存字面量 SemVer，不使用
  `version.workspace = true`。
- 四个初始 crate 分别从 `0.1.0` 开始，之后可以独立升级。
- 当前所有 crate 使用 `alpha` release channel；在项目明确进入下一发布阶段前保持该通道。
- 影响一个或多个可发布包的变更应通过 `smif commit` 创建 changeset。
- 本地和 Agent 环境禁止执行 `smif version` 与 `smif publish`。
- 版本更新和发布只能由 GitHub Actions 中的 `semifold ci` 执行；该流程根据 changeset
  更新各包版本、包间依赖并发布已经完成版本变更的包。
- Semifold 配置必须通过 `smif init`、`smif config` 等 CLI 维护，不手工模拟其输出。
- Semifold 的 base branch 为 `main`，release branch 为 `release`。
- `semifold-status.yaml` 在面向 `main` 的 pull request 上报告 changeset 状态。
- `semifold-ci.yaml` 在推送到 `main` 后运行 `semifold ci`，由 Semifold 编排 version 或
  publish 阶段。生成的 workflow 可以使用 CLI 的长命令名 `semifold`，本地文档统一使用
  短命令名 `smif`。

本地允许的 Semifold 操作限于 changeset 创建、只读状态查询和配置维护，例如
`smif commit`、`smif status`、`smif config sync` 和 `smif config channel`。本地验证不得
以 dry-run 为理由调用 `smif version` 或 `smif publish`。

初始化迁移时，经用户明确授权，可以把 Cargo 自动生成的共享版本继承手工转换为独立的
`version = "0.1.0"`。初始化完成后，正常版本变更必须交给 Semifold，不再手工修改版本号。

## 5. Core API

以下 API 表达设计意图，具体命名可以在实现过程中调整：

```rust
pub enum PromptKind {
    Secret,
    Visible,
}

pub enum MessageLevel {
    Info,
    Error,
}

pub struct PromptId(u64);

pub enum GreeterEvent {
    Prompt {
        id: PromptId,
        kind: PromptKind,
        message: String,
    },
    Message {
        level: MessageLevel,
        text: String,
    },
    Authenticated,
    SessionStarted,
    AuthenticationFailed,
    Cancelled,
}

pub struct SessionCommand {
    // 字段不对不可信前端公开。
    command: Vec<String>,
    environment: Vec<String>,
}

impl GreeterClient {
    pub async fn connect(socket: impl AsRef<Path>) -> Result<Self>;
    pub async fn create_session(&mut self, username: String) -> Result<()>;
    pub async fn respond(
        &mut self,
        prompt: PromptId,
        response: Secret,
    ) -> Result<()>;
    pub async fn cancel(&mut self) -> Result<()>;
    pub async fn start_session(
        &mut self,
        command: SessionCommand,
    ) -> Result<()>;
    pub async fn next_event(&mut self) -> Result<GreeterEvent>;
}
```

`PromptId` 由 core 生成，用来拒绝：

- 对已经回答过的 prompt 再次回答。
- 页面刷新后提交的旧回答。
- 在当前状态下无效的并发提交。

`Secret` 应隐藏 `Debug`/`Display` 内容，并在 drop 时尽力清零其 Rust 侧内存。

## 6. 认证状态机

建议的内部状态如下：

```text
Disconnected
    │ connect
    ▼
Idle
    │ create_session(username)
    ▼
Authenticating
    ├── AuthMessage::Secret  ──► WaitingForSecret
    ├── AuthMessage::Visible ──► WaitingForVisible
    ├── AuthMessage::Info/Error ─► 自动确认并继续
    ├── Error::AuthError ───────► Failed
    └── Success ────────────────► Authenticated

WaitingForSecret / WaitingForVisible
    │ respond(prompt_id, value)
    └───────────────────────────► Authenticating

Authenticated
    │ start_session(trusted_command)
    ▼
StartingSession
    ├── Success ────────────────► Started
    └── Error ──────────────────► Failed

任意活动状态
    │ cancel
    ▼
Cancelling ─────────────────────► Idle
```

实现必须满足：

- 同一时刻至多有一个 greetd 请求等待响应。
- 收到 `Info` 或 `Error` 类型的认证消息时，发送
  `PostAuthMessageResponse { response: None }` 确认，并继续读取下一条响应。
- 不假设第一个 secret prompt 一定是密码。
- 认证成功和 session 启动成功是两个不同阶段。
- 在退出、页面失联或不可恢复错误时尽力发送 `CancelSession`。
- greetd 连接断开后不盲目重放 PAM 回答。

## 7. 前端协议

### 7.1 基本原则

- 协议显式携带主版本号。
- 请求具有唯一 ID，响应关联该 ID。
- 状态事件具有递增序号，便于丢弃旧事件。
- 只暴露完成登录 UI 所必需的操作。
- JSON schema 与 Rust 类型共同维护。
- 未知方法、未知字段和版本不兼容必须返回结构化错误。

示例请求：

```json
{
  "protocol": 1,
  "id": 12,
  "method": "auth.respond",
  "params": {
    "promptId": 7,
    "response": "123456"
  }
}
```

示例响应：

```json
{
  "protocol": 1,
  "id": 12,
  "ok": true
}
```

示例事件：

```json
{
  "protocol": 1,
  "sequence": 42,
  "event": "auth.prompt",
  "data": {
    "promptId": 7,
    "kind": "secret",
    "message": "Password:"
  }
}
```

### 7.2 建议开放的方法

- `state.get`
- `auth.begin`
- `auth.respond`
- `auth.cancel`
- `session.select`
- `power.request`

是否提供用户列表由系统配置决定。前端必须始终能够使用手工用户名输入，以兼容隐藏用户、
网络用户以及无法从 NSS/AccountsService 枚举的账户。

### 7.3 禁止开放的数据和能力

- greetd socket 路径或句柄。
- 任意 shell 命令。
- 任意 executable、argument 或 environment。
- 任意文件读取和目录遍历。
- 任意 URL 导航或网络代理。
- 原始 PAM 错误 description 的无条件透传。

## 8. 前端和主题

正式前端由用户通过配置提供。Fomalhaut 不要求 React、Vue、Svelte 或任何包管理器；
只要求配置目录最终包含浏览器可加载的静态资源。

示例配置：

```toml
[frontend]
path = "/etc/fomalhaut/themes/my-theme"
entrypoint = "index.html"

[frontend.security]
allow_network = false
allow_navigation = false
allow_devtools = false
```

可选的主题清单：

```toml
[theme]
name = "My Theme"
protocol = 1
entrypoint = "index.html"
```

主题加载规则：

- 对入口文件做规范化路径检查。
- 拒绝通过 `..`、符号链接或编码绕过访问主题根目录之外的文件。
- 根据扩展名提供固定 MIME 类型。
- 默认拒绝远程资源和非 Fomalhaut scheme。
- 对 HTML 设置严格 Content Security Policy。
- 主题加载失败时显示内置的最小故障页面。

内置故障页面只负责报告主题无法加载，不作为正式可定制主题，也不需要实现完整登录流程。

仓库可提供一个 minimal theme，但其定位仅为：

- 展示协议如何使用。
- 支持集成和截图测试。
- 帮助主题作者验证运行环境。

## 9. WebView 运行环境

首选方向是 Linux 原生 WebKit 方案：

- GTK4 + WebKitGTK，或
- WPE WebKit。

最终 crate 和绑定版本应通过原型验证后决定。选择标准包括：

- Wayland 原生运行能力。
- 自定义资源 scheme。
- 可拦截导航、新窗口和下载。
- JavaScript 到 Rust 的受控消息通道。
- renderer 进程隔离和 sandbox 支持。
- 发行版可用性及打包成本。
- 在 Cage 等 kiosk compositor 下的稳定性。

Electron 或完整 Chromium 不作为首选，因为它们会增加包体、内存、进程管理复杂度和攻击面。

## 10. greetd 与 Wayland 启动

WebView 需要图形环境。推荐让 greetd 启动一个极简 Wayland compositor，再由 compositor
启动 Fomalhaut。例如：

```toml
[terminal]
vt = 1

[default_session]
command = "dbus-run-session cage -s -mlast -d -- fomalhaut"
user = "greeter"
```

具体 Cage 参数需要在支持的最低版本上验证并写入安装文档。

认证及 `StartSession` 成功后，Fomalhaut 退出，kiosk compositor 随之退出，用户 session
由 greetd 管理。Fomalhaut 必须作为专门的低权限 `greeter` 用户运行，不应以 root 运行。

## 11. 安全模型

### 11.1 信任关系

受信任：

- greetd 及其 Unix socket。
- Fomalhaut 安装的 Rust 二进制和系统配置。
- 管理员明确配置的 session 目录及 desktop entry。

默认不信任：

- 主题中的 HTML、CSS 和 JavaScript。
- WebView 导航目标。
- 前端传来的所有字符串、ID 和操作顺序。
- desktop entry 中未经策略验证的字段。

管理员安装自定义主题意味着接受该主题可以读取用户在其页面中输入的内容，但不意味着主题
自动获得系统命令执行或 greetd socket 访问权限。

### 11.2 必须实施的防护

- 只以低权限 greeter 用户运行。
- 正式模式不监听 TCP。
- 不把 greetd socket 暴露给前端。
- 禁止外部导航、新窗口、下载和开发者工具。
- 默认禁止网络访问及远程资源。
- 使用严格 CSP。
- bridge 使用方法白名单和结构化反序列化。
- 限制用户名、回答和消息的最大长度。
- 防止重复提交和并发认证请求。
- 日志中不记录 PAM 回答、密码、token 或完整 IPC payload。
- `Debug`/`Display` 不泄露 secret。
- 页面刷新、崩溃和 host 退出时取消活动 session。
- 主题路径必须防止目录穿越和 symlink escape。
- 电源操作采用明确枚举和管理员配置，不接受前端命令行。

### 11.3 Web secret 的固有限制

Rust 后端可以使用 `zeroize` 等机制尽力清除 secret，但 JavaScript 字符串、DOM 输入元素和
WebView renderer 内存不保证可验证地清零。提交回答后，示例前端应立即清空输入框并释放引用，
但这不能提供原生安全缓冲区同等级别的保证。

该限制必须在用户文档和安全文档中明确说明。如果部署场景要求秘密内存可验证清除，应选择
基于 `fomalhaut-core` 的原生 UI host，而不是 Web host。

## 12. 配置原则

配置文件使用 TOML。建议默认路径为 `/etc/fomalhaut/config.toml`，但编译和测试不应依赖
该路径实际存在。

配置大类预计包括：

- 主题路径和入口。
- session 搜索目录及过滤策略。
- 是否列出本地用户。
- 是否保存上次用户或 session。
- WebView 安全策略。
- 允许的电源操作。
- 日志级别和日志目标。
- 开发/演示模式。

配置解析分成两个阶段：

1. 语法反序列化。
2. 语义验证和路径规范化。

无效安全配置应导致启动失败或回退到安全默认值，不能静默放宽限制。

## 13. 错误、日志和恢复

- 使用结构化错误类型区分配置、transport、协议、认证、session 和 WebView 错误。
- 面向用户的错误信息不直接等于内部错误或 PAM description。
- 日志默认不包含用户名以外的认证内容；是否隐藏用户名可进一步配置。
- secret 类型必须提供安全的 `Debug` 实现。
- WebView 无法启动或主题加载失败时显示最小故障页面并保留可诊断日志。
- 无法连接 `GREETD_SOCK` 时明确退出，避免呈现一个永远无法登录的假界面。
- panic hook 和正常退出路径都应尽力取消活动认证 session。

## 14. 测试策略

### 14.1 Core 单元测试

- 每个合法状态转换。
- 每个非法状态转换。
- 重复和过期 `PromptId`。
- `Info`/`Error` 自动确认。
- 多轮 secret/visible prompt。
- 认证失败后重新开始。
- session 启动成功和失败。
- socket 断开及取消。
- secret 的 `Debug`/`Display` 不泄漏内容。

### 14.2 greetd stub 集成测试

- 用户名 + 密码。
- 错误密码。
- 多因素认证。
- 无密码账户。
- 可见 prompt。
- 混合 info、error 和 prompt。
- 认证成功后启动可信 session。
- 页面/host 中途取消。

### 14.3 Session 测试

- X11 和 Wayland desktop entry。
- `Hidden`、`NoDisplay` 和重复项。
- 无效或缺失的 `Exec`。
- session ID 稳定性。
- 路径穿越、符号链接和目录优先级。

### 14.4 Frontend protocol 测试

- JSON schema 和 Rust 类型一致。
- 协议版本不兼容。
- 未知方法和字段。
- 消息长度限制。
- 并发、重复和乱序请求。
- 前端永远无法提供实际 session command。

### 14.5 WebView 集成测试

- 自定义 scheme 和 MIME 类型。
- 外部导航、弹窗和下载被阻止。
- 默认网络访问被阻止。
- CSP 生效。
- 主题目录逃逸被拒绝。
- 页面刷新触发认证取消和状态重同步。
- Cage 下启动、登录和退出。

## 15. 开发和演示模式

为了让主题作者无需修改系统 greetd 配置即可开发，Fomalhaut 应提供 demo mode：

- 使用内存或 stub core。
- 可模拟各种 PAM prompt 和失败流程。
- 不访问真实 `GREETD_SOCK`。
- 不执行真实 session 和电源命令。
- UI 中必须明确标识当前为演示模式。

开发模式中开放网络或开发者工具必须由显式参数启用，并且不得成为正式 greetd 启动配置的
默认行为。

## 16. 兼容性与版本策略

- Rust crate 遵循语义化版本。
- 各 crate 独立维护版本，由 Semifold changeset 决定版本提升级别。
- 所有 crate 使用 Rust 2024 Edition，并跟随最新 Rust stable，不承诺固定 MSRV。
- Cargo manifest 不设置 `rust-version`；CI 不维护旧 Rust 版本兼容性矩阵。
- Rust stable 或依赖升级引起的必要技术变动，仍须先更新本文和 `TODO.md` 再实施。
- 第三方依赖跟随最新稳定版本，但通过 manifest 语义化约束和已提交 lockfile 保持构建可复现。
- 前端协议单独维护整数主版本。
- 同一 host 至少支持其当前协议版本。
- 破坏性前端协议变更必须增加主版本。
- 新增可选字段不应破坏旧主题。
- 主题清单声明所需协议版本；不兼容时显示清晰的故障页面。
- greetd IPC 兼容范围由 `greetd_ipc` 依赖版本和集成测试共同定义。

## 17. 待原型验证的决策

以下内容在完成小型原型后再固化：

- GTK4 + WebKitGTK 与 WPE WebKit 的最终选择。
- JavaScript bridge 的具体承载机制。
- 自定义 scheme 是否能在目标发行版上稳定提供所需 CSP 和 MIME 行为。
- renderer sandbox 在不同发行版中的默认状态和配置方法。
- 多显示器策略由 compositor 还是 Fomalhaut host 管理。
- 用户发现使用 NSS、AccountsService，还是作为可选 provider。
- session desktop entry 解析采用现有 crate 还是小型自有解析层。
- WebKitGTK、Cage 和 greetd 的最低兼容版本；Rust 工具链继续跟随 stable。

这些决策不得削弱本文定义的 core/UI 分离和前端权限边界。
