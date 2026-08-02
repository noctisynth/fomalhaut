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
│   │       └── protocol/
│   └── fomalhaut/
│       └── src/
│           ├── config.rs
│           ├── gtk_host.rs
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

#### 4.2.1 Discovery 与可信命令映射

session discovery 使用 `freedesktop-desktop-entry` 解析 Desktop Entry 基本格式和文件内
本地化字段，并在 Fomalhaut 内实现面向登录 session 的严格校验与 `Exec` 参数解析。依赖
禁用默认的 `gettext` feature，避免引入不必要的原生库和进程级 locale 行为；首阶段只采用
Desktop Entry 文件自身的 locale 值。选择现有解析 crate 是为了复用规范中的转义、分组和
locale 处理；登录 session 特有的安全策略仍由本项目掌握，不直接采用应用菜单的宽松默认
行为。

- 搜索目录由 host 以 `(path, SessionKind)` 有序提供；目录顺序就是优先级。默认目录和配置
  合并策略留在 host 配置层，session crate 不读取进程环境来隐式改变搜索范围。
- 每个目录只读取第一层且扩展名为 `.desktop` 的普通文件。目录不存在时忽略；目录存在但
  无法读取时返回可处理错误。
- `SessionId` 由 session 类型和 desktop 文件名确定，跨进程重建稳定，但其具体字符串格式
  不属于兼容 API，前端必须将其视为不透明值。ID 不包含绝对路径。
- 相同 `SessionId` 只处理最高优先级目录中的第一个文件；即使该文件被隐藏或校验失败，
  也不回退到低优先级同 ID 文件，避免选择结果随错误类型产生意外变化。
- 只接受 `Type=Application`、非空 `Name` 和非空且可解析的 `Exec`。`Hidden=true`、
  `NoDisplay=true`、无效布尔值、无效 UTF-8、无效 `TryExec` 和不支持的 `Exec` field code
  均拒绝进入可选列表。
- `Exec` 按 Desktop Entry 的双引号和反斜杠规则转换为 argv；登录 session 不需要文件或
  URL 参数，因此除表示字面量百分号的 `%%` 外拒绝所有 field code。绝不通过 shell 解释
  `Exec`。
- 绝对 `TryExec` 必须存在且可执行；相对 `TryExec` 只在 host 明确提供的 executable search
  path 中解析，不隐式信任前端或主题提供的路径。
- catalog 对外只公开 `SessionId`、本地化显示名和 `SessionKind`。解析出的 argv、desktop
  文件路径和环境变量保持在可信 Rust 侧；只有 `SessionCatalog::command` 能把当前 catalog
  中的 ID 转换为 `fomalhaut_core::SessionCommand`。
- 生成命令时根据 session 类型设置 `XDG_SESSION_TYPE`，并根据文件名和可选
  `DesktopNames` 设置 `XDG_SESSION_DESKTOP`、`DESKTOP_SESSION` 和
  `XDG_CURRENT_DESKTOP`。X11 wrapper 等发行版策略由后续 host 配置层在可信侧组合。
- discovery 返回可用 catalog 和逐项拒绝诊断；单个损坏文件不阻止其他 session 被发现，
  但目录级 I/O 失败不会被静默吞掉。

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
- 四个初始 crate 分别从 `0.1.0-alpha` 开始，之后可以独立升级。
- 当前所有 crate 使用 `alpha` release channel；在项目明确进入下一发布阶段前保持该通道。
- 影响一个或多个可发布包的变更应通过 `smif commit` 创建 changeset。
- changeset 的名称、分类和摘要必须使用英文，确保发布记录面向统一的国际化读者。
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
`version = "0.1.0-alpha"`。初始化完成后，正常版本变更必须交给 Semifold，不再手工修改
版本号。

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
- 在正常退出、页面失联或 host 可控的错误路径中，必须显式等待 `cancel()` 完成并发送
  `CancelSession`。
- Rust `Drop` 不执行异步 IPC、不阻塞 runtime，也不派生无法等待的后台取消任务；析构只
  清理敏感内存并关闭 transport。连接关闭是异常退出时的最后兜底。
- greetd 连接断开后不盲目重放 PAM 回答。

## 7. 前端协议

### 7.1 基本原则

- 协议显式携带整数主版本号；首个版本固定为 `1`。
- 请求具有唯一 ID，响应关联该 ID。请求 ID 和事件 sequence 必须是不大于
  `9_007_199_254_740_991` 的非负整数，以便 JavaScript 精确表示。
- 状态事件具有单调递增 sequence，便于丢弃旧事件；sequence 耗尽是 host 的不可恢复错误，
  不允许回绕。
- 只暴露完成登录 UI 所必需的操作。
- JSON Schema Draft 2020-12 从 Rust wire 类型确定性生成并提交到
  `protocol/v1.schema.json`；测试比较生成结果与提交文件，防止两者漂移。
- 未知方法、未知字段、未知 enum 值和版本不兼容必须被严格拒绝并转换为结构化错误。
- 单条 JSON 消息最大 128 KiB；用户名最大 256 bytes，认证回答最大 16 KiB，session ID
  最大 256 bytes，session 显示名最大 256 bytes，prompt/message 最大 4 KiB。状态快照最多
  暴露 128 个 session 和 16 条近期认证消息。
- 所有长度按 UTF-8 bytes 计算。用户名、session ID 和认证回答拒绝 NUL；用于标识符的文本
  还拒绝控制字符。
- JSON Schema 标准的 `maxLength` 按 Unicode 字符数而非 UTF-8 bytes 计算，因此 wire schema
  不使用该关键字表达 byte 上限，而使用 `x-fomalhaut-maxUtf8Bytes` 注解公开实际边界；Rust
  解码器中的 byte 长度校验是强制执行点。固定长度数组等按条目计数的边界仍使用标准
  `maxItems`。
- 认证回答在 Rust wire 类型中使用独立的 zeroizing 类型，其 `Debug`/`Display` 必须脱敏，
  并直接转换为 `fomalhaut_core::Secret`，不得经过可记录的通用字符串接口。
- 解析入口先检查消息总长度，再解析严格的公共 envelope，最后按 method 解析具体 params；
  不直接向调用方暴露宽松的 `serde_json::Value`。

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
  "ok": true,
  "result": {}
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

- `state.get`：无参数，返回完整公开状态快照。
- `auth.begin`：接收用户名。
- `auth.respond`：接收当前 `promptId` 和 zeroizing 回答。
- `auth.cancel`：无参数。
- `session.select`：只接收不透明 session ID。
- `power.request`：只接收 `poweroff`、`reboot` 或 `suspend` 枚举；在管理员策略完成前始终
  返回 `method_disabled`。

请求保持顶层 `{ protocol, id, method, params }` 形式。响应保持顶层
`{ protocol, id, ok, result }` 或 `{ protocol, id, ok, error }` 形式，且只能通过构造器建立
success/error 不变量。无法解析出请求 ID 的畸形 JSON 不生成一个伪造 ID 的响应，由 bridge
记录脱敏诊断并丢弃；已经解析出 ID 的错误必须关联原请求。

公开状态快照包含：认证状态、当前 prompt（如有）、有限数量的近期 info/error 消息、可选
session 摘要、当前选择的 session ID 和 capability。session 摘要只有 ID、显示名和 X11 /
Wayland 类型。capability 中的 power action 列表在策略启用前为空。

v1 事件至少包含：

- `state.changed`
- `auth.prompt`
- `auth.message`
- `auth.succeeded`
- `auth.failed`
- `auth.cancelled`
- `session.selected`
- `session.started`

结构化错误 code 至少覆盖：JSON/长度错误、不支持的版本、无效请求、未知方法、无效参数、
非法状态、过期 prompt、未知 session、禁用方法和内部错误。错误 message 必须是脱敏且适合
展示的稳定类别文本，不透传 serde、greetd、PAM 或文件系统的原始错误内容。

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

应用宿主固定使用 GTK4 + WebKitGTK 6.0，并通过 Rust `gtk4` 与 `webkit6` 原生绑定直接调用。
当前阶段只实现和维护该宿主，不并行实现 WPE WebKit。

原型启用 `webkit6` 的 `gtk_v4_18` Cargo feature，使 WebKit 绑定与 GTK 的可访问性接口保持
一致；因此当前编译基线为 GTK 4.18 或更新版本。开发环境使用滚动最新版 GTK 4.22 和
WebKitGTK 2.52，WebKitGTK 的最终最低兼容版本仍需发行版验证后确定。

不使用 Tao、Wry 或 Tauri。它们提供的跨平台窗口与 WebView 抽象不是 Linux DM 的需求，且
当前 Linux 路径会引入 GTK3/WebKitGTK 4.1 兼容层、额外事件循环集成，并限制对 WebKitGTK
安全接口和进程生命周期的直接控制。Electron 或完整 Chromium 同样不采用，因为它们会增加
包体、内存、进程管理复杂度和攻击面。

crate 边界保持如下：

- `fomalhaut-web` 保存与具体 WebView 工具包无关的协议、bridge/controller 和主题资源策略，
  不依赖 GTK 或 WebKitGTK，使协议和业务逻辑仍可在无图形环境测试和被其他应用宿主复用。
- `fomalhaut` 是当前唯一可执行宿主，直接依赖 `gtk4`、`webkit6` 和 `fomalhaut-web`，负责
  GTK application/window、WebView 生命周期、原生信号与系统集成。
- GTK 和 WebKit 对象只在创建它们的 GTK 主线程访问。WebView 回调不得阻塞等待 greetd；
  后续 Core 集成通过有界消息通道把请求交给异步 controller，再把序列化后的结果投递回
  GLib 主上下文。

首个应用侧原型使用内置的最小页面，不读取管理员主题目录，也不连接真实 greetd。它必须
验证以下宿主能力，验证通过后才能进入真实 greeter 集成：

- 创建 GTK4 全屏窗口并嵌入 WebKitGTK 6.0 `WebView`。
- 通过 `fomalhaut://theme/` 自定义 scheme 加载内置 HTML、CSS 和 JavaScript，不使用
  `file://` 或本地 TCP server。
- 在 WebKit `SecurityManager` 中只把 `fomalhaut` scheme 标记为 secure 和
  display-isolated。对照验证表明，自定义 scheme 的外部脚本无需 CORS-enabled 即可执行；
  此前的执行失败实际由 `nosniff` 兼容性问题导致。display-isolated 阻止其他 scheme 页面
  展示这些资源，严格 CSP 禁止网络连接，Rust 侧精确 URI 白名单拒绝未知 host/path。为保持
  最小权限，scheme 不标记为 CORS-enabled、local 或 no-access。
- 使用 WebKit `UserContentManager` 建立 JavaScript 到 Rust 的单一消息入口，所有输入先由
  前端协议 v1 解码；Rust 到 JavaScript 只投递序列化后的协议消息。
- 仅允许 `fomalhaut:` 页面和资源；拒绝 HTTP(S)、`file:`、`data:`、外部导航、新窗口与
  下载。WebView 设置默认关闭开发者工具、自动弹窗和非必要 Web 能力。
- 页面响应设置由 Rust 白名单决定的固定 MIME、严格 CSP、`Cross-Origin-Opener-Policy:
  same-origin` 和 `Cache-Control: no-store`。WebKitGTK 2.52 会在自定义 scheme 响应包含
  `X-Content-Type-Options: nosniff` 时拒绝执行该响应中的外部 JavaScript，即使其 MIME 已
  固定为 `application/javascript`；因此自定义 scheme 不发送 `nosniff`。这一兼容性例外
  不允许根据主题输入猜测 MIME，也不改变精确 URI 白名单、CSP 或 WebView 能力限制。
- WebKit 不把自定义 scheme 资源视为 CSP 的 `'self'`，因此 prototype CSP 仅为 script、style
  和 image 显式允许 `fomalhaut:`；`default-src`、`connect-src`、`frame-src`、`object-src`、
  `base-uri` 和 `form-action` 继续设为 `'none'`。该 scheme 的全部请求仍必须先通过 Rust
  侧精确 URI 白名单，CSP 允许 scheme 不代表允许任意 host 或 path。
- renderer 终止、页面刷新和窗口退出具有可观察且拒绝式的处理路径。

原型阶段可以用静态状态响应验证双向 bridge，但不得伪装成可用登录流程。真实 Core、Session、
配置和外部主题目录接入仍属于后续 Host 集成与主题资源任务。

在 Arch Linux、WebKitGTK 2.52.5、GTK 4.22.4 与 Cage 0.3.1 上的原型验证得到以下运行边界：

- WebKitWebProcess 由 WebKitGTK 通过 bubblewrap 启动；运行时观测到 `NoNewPrivs=1`、seccomp
  filter 生效且无 effective capabilities。6.0 API 不暴露 4.1 API 中的 sandbox 开关，宿主
  不尝试关闭 sandbox，也不为 renderer 增加额外文件系统路径。
- 宿主进程和 WebKitNetworkProcess 的该次观测未启用 seccomp。renderer sandbox 不能代替
  宿主侧协议校验和资源策略，NetworkProcess 也不能被视为无网络能力；正式模式继续依赖
  CSP、精确导航/响应白名单、临时 NetworkSession 和关闭非必要 Web API 来拒绝网络入口。
- Arch 运行时至少需要 `gtk4` 与 `webkitgtk-6.0`；Cage 是推荐的独立 kiosk compositor，
  不是 Rust 二进制的链接依赖。当前包版本的安装体积分别约为 54.67 MiB、130.77 MiB 和
  70.68 KiB，WebKitGTK 还依赖 bubblewrap、libseccomp、libsoup3、GStreamer 和图形栈。
- 一次调试构建空闲快照中，宿主、NetworkProcess 与 WebProcess 的 RSS 分别约为 382 MiB、
  157 MiB 和 402 MiB；RSS 会重复计算共享页，并显著受 debug 符号、GPU 驱动和主题影响，
  因而这里只作为原型成本上界信号。发布构建的 PSS/峰值以及非 Arch 发行版的包名和可用
  版本仍需单独测量，不能据此声明最低运行需求。

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
- 正常退出路径应显式等待活动认证 session 取消；panic/abort 等无法等待异步 IPC 的路径
  通过关闭 transport 触发连接级清理，不在 panic hook 中启动后台异步任务。

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

- 自定义 scheme 是否能在目标发行版上稳定提供所需 CSP 和 MIME 行为。
- renderer sandbox 在不同发行版中的默认状态和配置方法。
- 多显示器策略由 compositor 还是 Fomalhaut host 管理。
- 用户发现使用 NSS、AccountsService，还是作为可选 provider。
- session desktop entry 基本格式使用 `freedesktop-desktop-entry`，登录 session 的严格
  `Exec` 校验和安全策略由 `fomalhaut-session` 实现。
- WebKitGTK、Cage 和 greetd 的最低兼容版本；Rust 工具链继续跟随 stable。当前开发依赖
  跟随滚动最新稳定版本，最低兼容版本只能在发行版验证后声明。

这些决策不得削弱本文定义的 core/UI 分离和前端权限边界。
