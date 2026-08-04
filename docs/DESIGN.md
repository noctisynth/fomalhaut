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

基础 GitHub Actions workflow 使用简短的 `Checks` job 名称，避免 check-run 名称重复罗列
内部步骤。Rust 构建通过 `Swatinem/rust-cache@v2` 缓存 Cargo registry、Git 依赖和 workspace
target；Bun canary 可执行文件由 `oven-sh/setup-bun@v2` 自身缓存，npm package 下载缓存则由
`actions/cache@v5` 单独缓存 `~/.bun/install/cache`，并以 OS 与 `bun.lock` 内容生成 key。
不得缓存整个 `node_modules`，依赖树仍必须由 `bun install --frozen-lockfile` 从 lockfile 重建。

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
├── packages/
│   └── fomalhaut-sdk/
│       ├── package.json
│       ├── tsconfig.json
│       └── src/
│           ├── generated/
│           │   └── v1/
│           ├── bridge.ts
│           ├── client.ts
│           ├── errors.ts
│           ├── events.ts
│           └── index.ts
├── biome.json
├── bun.lock
├── package.json
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

### 4.5 Host controller 与线程边界

真实认证接入采用两层实现，保持 controller 可在无图形环境测试：

- `fomalhaut-web::controller` 持有 `GreeterClient<T>`、公开状态快照、当前 core `PromptId` 和
  事件 sequence。它接收已经严格解码的 `RequestEnvelope`，输出一个关联
  `ResponseEnvelope` 和按 sequence 排序的 `EventEnvelope` 列表，不依赖 GTK/WebKit。
- `fomalhaut` 在专用 OS 线程中运行单线程 Tokio runtime，并在该线程内创建
  `GreeterClient<UnixTransport>`。GTK/WebKit 对象及 `ScriptMessageReply` 始终留在 GTK 主
  线程；两侧只通过容量固定的同步通道交换可发送的类型。

通道与页面生命周期遵循以下规则：

- GTK 主线程使用非阻塞发送，队列已满时立即向当前请求返回脱敏的 `internal` 错误，不阻塞
  UI，也不创建无界任务。首阶段同时只允许一个未完成 bridge 请求；并发请求被拒绝。
- 每次 `LoadEvent::Started` 都递增页面 epoch、拒绝旧页面尚未完成的 reply，并按通道顺序请求
  controller 取消活动认证。controller 输出携带发起请求时的 epoch；GTK 丢弃与当前页面不
  匹配的输出，防止刷新后的旧响应或事件进入新文档。
- controller 对一个请求的处理是串行事务：调用一次 core 操作，排空该操作产生的 core
  event，先生成必要的 `state.changed`，再生成 prompt/message/succeeded/failed/cancelled
  事件，最后把响应和事件作为一个输出批次交回 GTK。
- 正常窗口退出、renderer 终止和 host 关闭都会发送 shutdown。worker 在退出前检查
  `needs_cancel()`，需要时显式等待 `cancel()`；线程 join 完成后宿主才结束。异常 abort 仍只
  能依靠连接关闭兜底。
- 启动必须读取非空 `GREETD_SOCK`。变量缺失、路径无效、runtime 创建失败、连接失败或 worker
  提前退出都是致命错误，宿主以非零状态明确退出，不继续显示无法认证的页面。

bridge 在 GTK 主线程完成消息总长度检查和严格协议解码，再把 typed request 移入有界队列；
不得把包含认证回答的原始 JSON 复制到跨线程队列。`auth.respond` 使用页面提供的数值只与
controller 保存的当前 core `PromptId` 比较，实际调用 core 时传回原 core ID，不允许前端
构造 core ID。

可信 session 接入继续保持相同的 crate 边界：

- `fomalhaut` 在启动 worker 前运行 `fomalhaut-session` discovery，并把 catalog 中每个条目
  转换为一组前端安全的 `SessionSummary` 和 Rust 内部 `SessionCommand`。主题只能看到摘要；
  命令、参数、环境变量和 desktop entry 路径不进入 JSON 或 GTK/WebKit 对象。
- 在配置层完成前，host 使用固定且不受进程环境影响的默认目录：按优先级读取
  `/usr/local/share/wayland-sessions`、`/usr/share/wayland-sessions`、
  `/usr/local/share/xsessions` 和 `/usr/share/xsessions`；相对 `TryExec` 只在
  `/usr/local/bin` 与 `/usr/bin` 中解析。不存在的目录继续忽略，目录级 I/O 错误、协议上限
  无法容纳 catalog 或没有任何可用 session 都是启动失败。后续配置层替换这组显式输入，
  不通过 `XDG_DATA_DIRS` 隐式改变可信搜索范围。
- controller 持有已经由 host 解析的可信 session 集合，并默认选择 catalog 稳定顺序中的
  第一项。`session.select` 只在该集合内按不透明 ID 切换选择，成功后发出
  `session.selected`；未知 ID 返回 `session_not_found`。选择可以在认证开始前或认证进行中
  调整，但 session 已开始、正在启动/取消或连接断开时拒绝改变。
- 前端协议不增加 `session.start`。任一认证请求使 core 进入 `Authenticated` 后，controller
  必须在同一个串行事务内立即使用当前选择的可信 `SessionCommand` 调用 `start_session`，
  排空 `Authenticated` 与 `SessionStarted` 事件后才把批次交回 GTK。这样不存在由不可信
  页面在认证成功后替换命令或延迟启动的窗口。
- worker 输出显式携带“session 已启动”终态，不通过检查 JSON 文本推断。GTK 先完成当前
  reply 和有序事件的投递，再以成功状态关闭 worker 和 application；此时 core 已处于
  `Started`，shutdown 不再发送 `CancelSession`。Fomalhaut 的零退出使作为父进程的 Cage
  随之退出，greetd 在 `StartSession` 成功后接管用户 session。Cage/greetd 的完整交接仍需
  在真实 DM 环境做端到端验证，Unix socket stub 只验证 IPC 与宿主退出信号。

当前纵向切片已使用内存 transport 和真实 Unix socket stub 验证密码 prompt、认证成功、认证
失败、过期 prompt、显式 `auth.cancel`、页面取消、关闭取消、transport 断开脱敏、事件顺序
及 session/power 禁用状态。Wayland 实例也已通过只保持连接的 stub 验证页面 `state.get`
确实经过 WebKit bridge、有界通道和真实 controller；缺少或无法连接 `GREETD_SOCK` 均返回
非零状态。可信 session 切片进一步验证了默认选择、未知 ID 拒绝、选择事件、启动失败脱敏，
以及 Unix socket 上发送的 `StartSession` argv/env 与 host 解析结果完全一致；worker 仅在收到
greetd `Success` 后输出 session-started 终态。真实 Cage 退出与 greetd 接管不由 stub 结果
替代，继续作为系统端到端验证项。

### 4.6 Monorepo 版本与发布

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
- 所有 Rust package 必须提供 crates.io 接受的非空 `description` 和
  `license`/`license-file`；Fomalhaut 统一继承 `AGPL-3.0-only` license 与仓库 URL，各 crate
  保留与自身职责对应的独立 description。`repository` 虽不是 crates.io 的硬性发布字段，仍
  必须提供，确保注册表元数据可追溯到源码。
- 所有 npm 发布 package 必须提供合法且非 private 的 `name`、SemVer `version`；同时维护
  `description`、`license`、`repository`、明确的 `exports` 和 `files` 白名单。根 workspace
  package 必须保持 private，不得发布。
- `semifold-ci.yaml` 在运行 `semifold ci` 前必须使用 Bun frozen lockfile 安装依赖并构建
  `fomalhaut-sdk`，确保 npm `files = ["dist"]` 的发布内容在全新 runner 中真实存在。发布前可
  本地运行 `cargo package` 和 `npm pack --dry-run` 检查 payload，但仍严禁本地执行
  `cargo publish`、`npm publish`、`smif version` 或 `smif publish`。
- `semifold-ci.yaml` 必须与基础 CI 一样固定使用 `ubuntu-26.04` 并安装
  `libwebkitgtk-6.0-dev`，让 `cargo publish` 对最终 `fomalhaut` crate 的 tarball 验证能够
  获得 GTK 4.18+ 以及对应的 GLib、WebKitGTK pkg-config metadata；`ubuntu-latest` 当前提供的
  GTK 4.14 不满足 `webkit6/gtk_v4_18`，不得通过跳过 Cargo verification 掩盖构建基线不匹配。
- Semifold 配置必须通过 `smif init`、`smif config` 等 CLI 维护，不手工模拟其输出。
- Semifold 的 base branch 为 `main`，release branch 为 `release`。
- `semifold-status.yaml` 在面向 `main` 的 pull request 上报告 changeset 状态。
- `semifold-ci.yaml` 在推送到 `main` 后运行 `semifold ci`，由 Semifold 编排 version 或
  publish 阶段。生成的 workflow 可以使用 CLI 的长命令名 `semifold`，本地文档统一使用
  短命令名 `smif`。
- `semifold-status.yaml` 与 `semifold-ci.yaml` 必须通过 `setup-semifold` 的 `version` 输入
  pin 到和仓库本地开发环境一致的 Semifold CLI 版本。该 action 输入使用带 `v` 的发布版本
  格式，因此本地 `semifold 0.3.0-rc.1` 对应 `v0.3.0-rc.1`；不得依赖 action 的 latest
  release 默认值。升级 Semifold 时必须在同一变更中同步两处 workflow 并验证本地版本。

本地允许的 Semifold 操作限于 changeset 创建、只读状态查询和配置维护，例如
`smif commit`、`smif status`、`smif config sync` 和 `smif config channel`。本地验证不得
以 dry-run 为理由调用 `smif version` 或 `smif publish`。

初始化迁移时，经用户明确授权，可以把 Cargo 自动生成的共享版本继承手工转换为独立的
`version = "0.1.0-alpha"`。初始化完成后，正常版本变更必须交给 Semifold，不再手工修改
版本号。

### 4.7 Arch Linux 与 AUR 发布

Arch Linux 的首个发行版包名为 `greetd-fomalhaut`，由 AUR 从 Fomalhaut 的正式源码标签构建。
它是版本化源码包，不使用 `-git` 或 `-bin` 后缀。AUR 发布只跟踪最终应用 package 的
`fomalhaut-v*` 标签，不跟踪 `fomalhaut-core`、`fomalhaut-session`、`fomalhaut-web` 或
`fomalhaut-sdk` 的独立标签；只有最终应用已经由 Semifold 完成发布并产生标签后，才能生成
对应的 AUR 版本。

上游 SemVer 与 Arch 版本分别保存：`_upstream_ver` 保留标签和 Cargo 使用的原始版本，例如
`0.1.0-alpha.0`；`pkgver` 将其中 Arch 禁止的连字符转换为点，例如
`0.1.0.alpha.0`。新上游版本从 `pkgrel=1` 开始。仅修复 AUR 打包而不发布新 Fomalhaut 版本
时，通过手动触发发布流程、显式指定同一标签和更高 `pkgrel` 完成，不修改 Cargo package
version，也不在本地执行 Semifold version/publish。自动任务发现 AUR 已有相同 `pkgver` 时，
无论其当前 `pkgrel` 是多少都必须 no-op，避免把已经修订到更高 `pkgrel` 的包降级；手动任务
对相同 `pkgver` 只接受严格高于 AUR 当前值的 `pkgrel`。

`greetd-fomalhaut` 的标准部署以 greetd、Cage 和 Fomalhaut 组成完整图形登录链路。greetd
不提供 Wayland compositor，而当前受支持且已经端到端验证的启动命令固定使用 Cage，因此
`greetd` 与 `cage` 都是必需运行时依赖，不是 optional dependency。标准命令直接调用
`dbus-run-session`，因此提供该命令的 `dbus` 同样是必需依赖。包同时依赖 Arch 的 `gtk4` 与
`webkitgtk-6.0`。发布构建直接链接的 ABI 必须按当前 Arch 提供者显式声明；当前还包括
`glib2`、`glibc`、`libgcc` 和 `libsoup3`，不得用聚合包或偶然的传递依赖替代。后续根据干净
Arch 构建、ELF `NEEDED` 和 `namcap` 结果滚动维护；构建依赖使用 Arch 的 `cargo`。
`accountsservice` 只提供默认 `auto` 用户发现中的显示名和头像增强，缺失时仍能通过 glibc
提供的 `/usr/bin/getent` 完成 NSS fallback，因此声明为 `optdepends` 而不是必需依赖。包安装
`/usr/bin/fomalhaut`、上游许可证、配置文档和一份使用
`/usr/bin/fomalhaut` 的 greetd/Cage 示例，但不得覆盖管理员的 `/etc/greetd/config.toml` 或
`/etc/fomalhaut/config.toml`。

许可证边界分为两层：Fomalhaut 源码和安装后的软件继续使用 `AGPL-3.0-only`，AUR
`PKGBUILD` 的 `license` 字段也必须声明 `AGPL-3.0-only`；独立 AUR Git 仓库中的
`PKGBUILD`、`.SRCINFO` 和随包提供的打包元数据使用 Arch 推荐的 `0BSD`，以保留未来进入
官方仓库的资格。0BSD 只授权打包脚本，不重新许可 Fomalhaut 源码或二进制。上游仓库中的
AUR 模板和随附的 0BSD 文件必须清楚标明这一作用范围。

AUR 发布由独立的 GitHub Actions workflow 承担，并遵守以下边界：

- workflow 在 `Semifold CI` 成功结束后运行，也允许管理员手动指定
  `fomalhaut-v*` 标签与 `pkgrel` 重新发布打包修订。由于使用 `GITHUB_TOKEN` 的 workflow
  所创建的标签不会可靠触发另一个 tag workflow，AUR 流程不得只依赖 tag push 事件。
- 自动流程解析最新 `fomalhaut-v*` 标签，确认标签中的 `crates/fomalhaut` 版本一致且该版本
  已经能从 crates.io 查询到；随后比较 AUR 当前 `greetd-fomalhaut` 版本，相同版本直接
  no-op，不重复请求发布审批。
- 发布前使用默认分支上的最新打包模板，在干净的 Arch Linux 环境渲染具体 `PKGBUILD`，
  生成 `.SRCINFO`，使用锁文件和 `--frozen` 构建、运行测试，并使用 `namcap` 检查 recipe
  与产物。实际被构建的源码仍严格来自选定的 `fomalhaut-v*` 标签 tarball，该 tarball 必须
  使用计算出的 SHA-256，不允许 `SKIP`。这样打包修订可以修复旧上游版本的 recipe，而不要求
  release tag 预先包含后来新增的发布工具。
- 验证产物通过 artifact 传递给发布 job。发布 job 必须绑定受保护的
  `aur-production` GitHub Environment，在人工批准后才使用专用 AUR SSH key 克隆并推送
  `ssh://aur@aur.archlinux.org/greetd-fomalhaut.git`；AUR 仓库不作为主仓库 subtree 管理。
- AUR maintainer 名称和邮箱使用 GitHub Environment/Repository variables 提供，专用 SSH
  私钥使用 Environment secret 提供。`aur.archlinux.org` 的官方 Ed25519 主机密钥指纹固定在
  受代码审查的 workflow 中；运行时可以用 `ssh-keyscan` 自动取得完整公钥，但必须先计算
  SHA-256 指纹并与固定值进行唯一匹配，匹配成功后才能把扫描结果用作 `known_hosts`。扫描
  失败、出现多个不同指纹或指纹不匹配都必须 fail closed，不能禁用严格主机密钥检查。AUR
  轮换主机密钥时，应先根据官方页面或公告核验新指纹，再通过普通代码评审更新 workflow，
  不需要额外维护 known-hosts secret。workflow 不在日志中输出私钥，不代表用户在本地创建
  AUR package，也不绕过 AUR 的 maintainer 审核责任。

### 4.8 源码工作区安装器

仓库根目录提供可执行的 `install.sh`，用于开发机从当前 checkout 构建并安装 Fomalhaut 与
React 参考主题。它不是 AUR/package manager 的替代品，也不参与发布版本计算；重复运行必须
同时支持首次安装和原地更新。

安装器遵守以下安全边界：

- Cargo、Bun 安装与前端构建始终以调用者的普通用户身份执行；脚本拒绝直接由 root 启动，
  只在写系统目录、原子切换文件和可选重启 greetd 时调用 `sudo`。
- 写真实系统前必须确认固定 greetd 命令引用的 `/usr/bin/dbus-run-session`、`/usr/bin/cage`
  可执行，且配置的 greeter 账户可由系统账户数据库解析；验证失败不得生成不可启动的配置。
- Rust 使用 `cargo build --release --locked -p fomalhaut`，前端先执行
  `bun install --frozen-lockfile` 再调用 workspace 的 `build:theme`，不得隐式更新 lockfile。
- 二进制先写入同目录临时文件，保留现有文件的带时间戳备份后通过 rename 切换。主题每次安装
  到只读 release 目录，`/etc/fomalhaut/themes/nocturne` 使用相对 symlink 原子指向新 release；
  既有普通目录首次迁移时保留为 `legacy` 备份，不递归删除旧主题或 release。
- `/etc/fomalhaut/config.toml` 与 `/etc/greetd/config.toml` 不允许用 `sed`/正则盲目覆盖整份
  文件。内置 updater 必须先用 Python 标准库 `tomllib` 验证旧内容，只修改脚本拥有的 table/key，
  再验证新 TOML 和预期值；现有文件先生成同目录时间戳备份，临时文件继承 mode/owner，并用
  同文件系统 `os.replace` 与 fsync 原子提交。为避免原子替换悄然改变链接语义，配置文件为
  symlink 或其他非普通文件时必须拒绝修改。两个配置目标的类型和现有 TOML 必须在切换二进制、
  主题或任一配置前完成 preflight；无法解析、重复目标 key 或验证失败必须 fail closed，不能留下
  已知可提前避免的部分安装。
- Fomalhaut 配置只维护 `[frontend].path`，并在明确传入缩放参数或首次创建文件时维护
  `[display].scale`；其他 section 和注释尽量原样保留。greetd 配置只维护
  `[default_session].command` 与 `user`，命令使用绝对二进制路径、Cage 和独立
  `XCURSOR_SIZE`，不再注入 `GDK_SCALE`。
- 默认不重启 display manager，避免意外终止当前图形会话；只有显式 `--restart` 才调用
  `systemctl restart greetd`。`--system-root` 允许在临时根目录验证完整安装和配置更新而不写
  主机 `/etc` 或 `/usr`。

### 4.9 用户发现与头像资源

用户发现是 Linux 宿主集成，不属于 greetd IPC core。首阶段在最终 `fomalhaut` crate 中以
内部 provider trait 隔离系统来源，并把已经过滤、验证的公开摘要交给 `fomalhaut-web`
controller；provider 稳定并出现其他宿主复用需求后，再评估提取独立 crate，不能为抽象而让
`fomalhaut-core` 依赖 D-Bus、NSS 或文件系统。

`/etc/fomalhaut/config.toml` 增加严格的 `[users]` 配置：

```toml
[users]
provider = "auto"
```

provider 只接受 `auto`、`accounts_service`、`nss` 和 `none`，默认 `auto`。`auto` 优先调用
system bus 上的 AccountsService `ListCachedUsers`；只有 system bus/服务不可用、无法激活、
连接中断、调用超时、接口不兼容或顶层响应无法解析时才回退 NSS。明确的 D-Bus
`AccessDenied` 不得回退，以免绕过管理员账户可见性策略；AccountsService 成功返回空列表、
单个用户属性读取失败或全部条目被过滤时同样不得为了填充界面而回退。`accounts_service` 与
`nss` 固定单一来源，`none` 禁用枚举。任何 provider 失败、超时或单个账户不合法都不能阻止
greeter 启动：公开列表可以为空，手工用户名输入必须始终可用。整个发现任务在认证 worker
初始化阶段的独立线程执行，并设有限时；GTK 主线程不得等待 D-Bus、NSS 或头像 I/O。

AccountsService 只读取用户对象的 `Uid`、`UserName`、`RealName`、`IconFile`、
`SystemAccount`、`Locked` 和可用于稳定排序的登录元数据。排除 system/locked account，跳过
空、非 UTF-8、重复、越过协议上限的条目；显示名为空时回退到用户名。NSS fallback 固定以
绝对路径、无 shell、固定参数执行 `/usr/bin/getent passwd`，让 NSS 的进程级枚举状态隔离在
可终止的子进程中；前端和配置不得提供 executable、参数或环境。宿主清理继承环境，限制执行
时间和标准输出总量，超时、超限、非零退出或非 UTF-8 输出时终止并回收子进程，且不得接受
部分结果。解析结果使用 `/etc/login.defs` 的 `UID_MIN`/`UID_MAX`（读取失败时使用
1000/60000 安全默认值）筛选普通账户，排除明确的 `nologin`/`false` shell，并把用户名同时
作为显示名。最终列表去重、确定性排序并限制为 128 项。Fomalhaut 继续禁止生产代码中的
`unsafe`，不得为进程内 `getpwent` 枚举放宽 workspace lint。

前端公开用户类型为 `{ username, displayName, avatarUrl }`。用户列表是页面初始恢复状态的一
部分，因此作为必填 `users` 数组直接加入 `state.get` 的 `StateSnapshot`，而不是引入独立
`users.list` 请求。主题选择摘要后仍只通过既有 `auth.begin(username)` 开始认证；宿主不得因
摘要存在就假定账户仍有效，greetd/PAM 继续是认证权威。

AccountsService 的 `IconFile` 是宿主路径，绝不能直接作为 `file://`、原始路径或文件读取 API
暴露给主题。头像代理遵守以下边界：

- 使用不跟随最终 symlink 的只读文件句柄打开候选头像，随后基于该句柄检查普通文件、所有者、
  长度和实际 `/proc/self/fd` 目标；只有文件属于对应 UID，或真实目标位于受信任的
  `/var/lib/AccountsService/icons` 根内时才接受，避免把 greeter 可读的任意文件转成主题资源。
- 单头像最多 2 MiB，只接受由固定 magic bytes 识别的 PNG、JPEG 或 WebP 栅格数据；拒绝 SVG、
  HTML、扩展名推断和 AccountsService 提供的 MIME。读取使用打开后的同一文件描述符并再次
  限长，避免检查与读取不同对象或无界增长。
- 有效头像在宿主内存中映射为不透明、不可枚举文件路径的 `fomalhaut://avatar/<id>`；
  `UserSummary.avatarUrl` 只有成功代理时才存在。现有 scheme handler 精确区分 `theme` 与
  `avatar` host，只允许 GET 和已注册 ID，返回固定 MIME、`Cache-Control: no-store`，不开放
  目录、原始路径、上传或任意读取。
- NSS 用户、缺失头像和任何验证失败都返回 `avatarUrl = null`。失败日志只报告稳定类别，不
  输出用户名、UID、IconFile、真实目标或图像内容。头像不是登录必要条件。

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
    ├── Error::AuthError ───────► Cancelling ── CancelSession::Success ──► Failed
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
- Draft 2020-12 的规范 metaschema URI 固定为
  `https://json-schema.org/draft/2020-12/schema`。该 URI 由 JSON Schema 官方文档使用，且
  返回 `application/schema+json`；编辑器因 MIME、网络、缓存或 dialect 支持问题无法下载时，
  应通过编辑器 schema catalog、缓存或本地映射解决，不得把使用 2020-12 dialect 生成的文档
  伪装成 Draft-07。
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
- `power.request`：只接收 `poweroff`、`reboot` 或 `suspend` 枚举。宿主只接受管理员配置
  allowlist 与 systemd-logind 当前无交互授权能力的交集；不在 capability 中的动作返回
  `method_disabled`，执行失败返回脱敏的 `internal` 错误。

请求保持顶层 `{ protocol, id, method, params }` 形式。响应保持顶层
`{ protocol, id, ok, result }` 或 `{ protocol, id, ok, error }` 形式，且只能通过构造器建立
success/error 不变量。无法解析出请求 ID 的畸形 JSON 不生成一个伪造 ID 的响应，由 bridge
记录脱敏诊断并丢弃；已经解析出 ID 的错误必须关联原请求。

公开状态快照包含：认证状态、当前 prompt（如有）、有限数量的近期 info/error 消息、经过
过滤的用户摘要、session 摘要、当前选择的 session ID 和 capability。用户摘要只有用户名、
显示名和可选的不透明头像 URL；session 摘要只有 ID、显示名和 X11 / Wayland 类型。
capability 中的 power action 列表由可信宿主生成。电源功能默认关闭；启用后，宿主通过系统
D-Bus 查询 systemd-logind 的 `CanPowerOff`、`CanReboot` 和 `CanSuspend`。只有返回 `yes` 的
动作才加入公开列表；`no`、`na`、`challenge`、D-Bus 不可用和查询失败都按不可用处理。
Fomalhaut 不运行 Polkit agent，也不为 greeter 发起交互授权。

收到已发布能力对应的请求时，controller 先取消仍在进行的 greetd 认证会话并清理 prompt，
再通过 systemd-logind 的 `PowerOff(false)`、`Reboot(false)` 或 `Suspend(false)` 执行动作。
这里的 `false` 明确禁止 D-Bus 方法发起交互授权。电源后端故障不得使 greeter 启动失败：启动
时退化为空 capability；请求与能力查询之间发生竞态时，调用失败只返回稳定、脱敏错误，不
回退到 `systemctl`、shell 或任意命令执行。

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
网络用户以及无法从 NSS/AccountsService 枚举的账户。用户摘要和头像不构成账户存在性、
可登录性或认证成功的证明。

### 7.3 禁止开放的数据和能力

- greetd socket 路径或句柄。
- 任意 shell 命令。
- 任意 executable、argument 或 environment。
- 任意文件读取和目录遍历。
- 任意 URL 导航或网络代理。
- 原始 PAM 错误 description 的无条件透传。

### 7.4 TypeScript SDK

Fomalhaut 把可由主题项目直接安装的 TypeScript SDK 作为正式下游包维护。npm 包名固定为无
scope 的 `fomalhaut-sdk`；不使用无法注册的 `@fomalhaut` scope。SDK 位于
`packages/fomalhaut-sdk`，属于 Bun workspace，不加入 Cargo workspace，并由 Semifold
的 Node.js resolver 独立维护版本和 `alpha` release channel。

Node/TypeScript 工具链统一使用 Bun，不维护 npm、pnpm 或 Yarn lockfile。根 `package.json`
以 `workspaces = ["packages/*"]` 发现 package，根 package 必须为 private；`bun install` 产生
并提交文本格式的 `bun.lock`，CI 使用 `bun install --frozen-lockfile`，禁止隐式迁移或同时提交
其他包管理器 lockfile。

包管理器约束对发布事务保留一个窄例外：Semifold CI 可使用其 Node.js resolver 默认生成的
`npm publish --provenance --access public`，以支持 npm trusted publishing/OIDC provenance。
npm 不参与依赖安装、workspace 解析、脚本、测试、构建或 lockfile 生成，本地与 Agent 也不得
执行 publish；该命令只能由 GitHub Actions 中的 `semifold ci` 间接调用。根 private package
不登记为 Semifold 发布包，只同步 `packages/fomalhaut-sdk`。

`fomalhaut-sdk` 首次发布已经完成，后续 npm 发布仅使用 trusted publishing/OIDC：workflow
保留 `id-token: write`，并通过 `actions/setup-node@v6` 提供支持 OIDC 的 Node.js 24/npm
运行时，但不得配置 `registry-url`、`.npmrc`、`NPM_TOKEN` 或 `NODE_AUTH_TOKEN`。setup-node
不得启用 npm package-manager cache，也不得引入 npm lockfile 或替代 Bun 的安装流程。

Fomalhaut 有意跟随 Bun 的滚动 canary，以使用稳定版尚未发布的 Rust 实现开发线；不把它写成
尚不存在的稳定 `1.4.0`。本地工具链必须是 `bun upgrade --canary` 所选择的 canary，GitHub
Actions 必须使用 `oven-sh/setup-bun@v2` 且显式设置 `bun-version: canary`，不得省略后回退到
`latest`，也不得填入 `1.4.0`。canary 会随 Bun `main` 更新，因此 CI 必须输出
`bun --version` 与 `bun --revision` 以记录实际验证的提交；`bun.lock` 只保证 npm 依赖解析可
复现，不宣称固定滚动 canary 可执行文件本身。

Rust wire 类型仍是协议的唯一事实来源。`fomalhaut-web` 使用 `ts-rs` 为请求、响应、事件、
状态、prompt、session 和结构化错误派生 TypeScript 类型。生成边界与 Rust 协议模块保持
一致：同一个 Rust 源文件中的公开 wire 类型合并到同一个 TypeScript 文件，而不是为每个
类型创建文件。首阶段固定映射为 `error.rs` → `protocol-error.ts`、`request.rs` →
`protocol-request.ts`、`message.rs` → `protocol-message.ts`、`secret.rs` →
`protocol-secret.ts`；模块内各类型通过相同的
`#[ts(export, export_to = "v1/protocol-request.ts")]` 目标由 `ts-rs` 原生合并并去重 import。
所有 `.ts`、`.tsx` 和生成 binding 文件名必须使用 ASCII `kebab-case`；不得使用
`PascalCase` 或 `camelCase` 文件名。TypeScript 类型和类名仍按语言惯例使用 `PascalCase`，
该命名约束只作用于文件和目录项。

`export_to` 只记录相对于 `TS_RS_EXPORT_DIR` 的稳定路径，不在 Rust 源码中硬编码跨越仓库的
`../../../packages/...` 路径。普通 Cargo 测试把自动导出定向到被忽略的 `target/ts-rs`，避免
测试修改源码树；显式 SDK 生成命令才把根目录覆盖为
`packages/fomalhaut-sdk/src/generated`。生成流程不得放在 `build.rs` 或正常 greeter 启动路径
中，也不得要求运行中的低权限 `greeter` 写开发产物。

生成边界必须特别处理：

- `RequestId`、`PromptId` 和 `Sequence` 虽由 Rust `u64` 承载，但已被协议限制为 JavaScript
  safe integer，导出时逐类型映射为 TypeScript `number`；不得全局把任意 `u64` 映射为
  `number`。
- `EmptyParams` 必须表达没有可接受字段的对象语义；若 `ts-rs` 生成宽泛的 `{}`，应覆盖为
  `Record<string, never>`。
- UTF-8 byte 上限、集合数量和其他不能由 TypeScript 静态表达的约束仍以 Rust 校验和
  `protocol/v1.schema.json` 为准；SDK 类型不能替代运行时协议验证。
- 生成结果必须提交，并由 CI 重新生成后检查 Git diff，防止 Rust、JSON Schema 和 SDK 类型
  漂移。

`fomalhaut-sdk` 在生成 wire types 上提供手写、框架无关的 Client。公开 API 至少覆盖
`state.get`、`session.select`、`auth.begin`、`auth.respond`、`auth.cancel`、
`power.request` 和带判别联合收窄的事件订阅。Client 的 `power.request` 只接受生成类型中的
`PowerAction`；主题必须先读取 `state.get` 的 capability，只展示其中存在的动作。Client 内部管理 request ID、验证
响应关联、协议版本和单调 event sequence，并把协议拒绝、bridge 失败和本地 busy 分成稳定
错误类型。

SDK 通过可注入的 `FomalhautTransport` 隔离宿主，默认 `WebKitTransport` 封装
`window.webkit.messageHandlers.fomalhaut` 与 `fomalhaut:event`。这允许 Node 单元测试、未来
demo transport 或其他宿主复用 Client。Client 同一时刻只允许一个请求，不自动排队认证回答，
避免 secret 因排队在 JavaScript 闭包中延长存活；SDK 不记录请求 body，主题仍须在提交后立即
清空输入元素。首阶段 SDK 保持零运行时依赖、纯 ESM，并由 TypeScript compiler 生成 JavaScript
和 declaration 文件，不引入 bundler。

所有手写和生成的 TypeScript 都由仓库锁定版本的 Biome 统一处理。生成命令先运行 `ts-rs`，
再对 generated 目录执行 Biome format，随后执行只读 check；CI 使用 Biome `ci`、TypeScript
typecheck、SDK 单元测试和 build，并在重新生成后以 Git diff 检查产物已提交。生成目录不得整体
关闭 linter；只能为生成器无法规避的问题添加有说明的最小规则 override。Biome 和 TypeScript
随项目滚动升级到最新稳定版，但每个提交必须通过精确依赖版本与 `bun.lock` 保持可复现。所有
脚本通过 `bun run` 调度，SDK 测试使用 `bun test`；首阶段 build 仍由 TypeScript compiler 生成
标准 ESM JavaScript 和 declaration，不为仅有的库代码额外引入 bundler。

## 8. 前端和主题

正式前端由用户通过配置提供。Fomalhaut 不要求 React、Vue、Svelte 或任何包管理器；
只要求配置目录最终包含浏览器可加载的静态资源。

示例配置：

```toml
[frontend]
path = "/etc/fomalhaut/themes/my-theme"
```

外部主题目录必须包含主题清单：

```toml
[theme]
name = "My Theme"
protocol = 1
entrypoint = "index.html"
```

主题加载规则：

- 外部主题是管理员选择的受信任代码，而不是安全沙箱中的不可信内容。主题 JavaScript 能读取
  用户在页面中输入的用户名、PAM 回答和其他认证信息，因此当前版本只适合安装来源可信、内容
  已审查的主题。资源 capability、CSP 和导航限制用于缩小误配置与文件暴露面，不构成对恶意
  主题代码的完整隔离；主题来源验证、签名或打包机制留待后续安全加固。
- `/etc/fomalhaut/config.toml` 不存在时使用内嵌 minimal theme；文件存在但无法读取、解析或
  通过语义验证时明确失败，不静默回退。配置指定外部主题时，缺失/损坏的 `theme.toml`、
  不支持的 protocol 或无效入口同样是启动失败。运行中某个资源消失只返回脱敏的资源错误。
- 外部主题根必须是绝对目录。host 使用 `cap-std` 打开一次目录 capability；主题清单和所有
  资源只通过该句柄的相对路径 API 打开，并直接从打开的文件描述符读取。不得先
  `canonicalize` 再按全局路径读取，避免检查与读取不同文件。
- URI 只接受 `fomalhaut://theme/` 下由 ASCII 字母、数字、`-`、`_`、`.` 和 `/` 组成的路径；
  每个 segment 必须非空且不是 `.`/`..`，拒绝反斜杠、百分号、query、fragment、绝对路径和
  NUL。根 URI 映射到清单入口，其他 URI 映射到同名相对文件。
- 配置的主题根路径本身可以是 symlink：打开后其目标目录成为 capability 根，支持管理员用
  symlink 选择实际主题位置。根内相对 symlink 可以引用仍位于 capability 内的共享资源；
  指向根外的绝对或相对 symlink 解析为不可用资源，不得把 `greeter` 可读的外部文件暴露给
  `fomalhaut://theme/`。这是当前实现和回归测试覆盖的行为；对不同平台、复杂链接链和并发替换
  的完整保证仍以 P2 的独立 `cap-std` 审计为准。
- 安全测试必须区分 `Err` 与“拒绝后按不存在处理”的 `Ok(None)`：根外 symlink 返回后者也
  表示内容未被读取。测试同时验证根内 symlink 可读和根外内容不会出现在响应中，避免把错误
  分类误判为目录逃逸。
- 从已打开文件句柄读取仍要验证普通文件并限制单资源最多 8 MiB。主题清单最多 16 KiB，系统
  配置最多 64 KiB。
- 根据小写扩展名提供固定 MIME 白名单，首阶段支持 HTML、CSS、JavaScript、JSON、SVG、
  PNG、JPEG、GIF、WebP、ICO、WOFF 和 WOFF2；未知扩展名拒绝，不根据文件内容或主题输入
  猜测 MIME。
- 默认拒绝远程资源和非 Fomalhaut scheme。
- 对 HTML 设置严格 Content Security Policy。
- 只允许清单入口作为顶层导航；其他 allowlist 资源只能作为子资源响应，主题不能导航到自己
  的其他 HTML 页面来重建授权上下文。
- 内置最小故障页面完成前，外部主题启动验证失败使宿主以非零状态退出；不得用内置登录主题
  静默替代管理员明确配置但损坏的主题。

内置故障页面只负责报告主题无法加载，不作为正式可定制主题，也不需要实现完整登录流程。

仓库可提供一个 minimal theme，但其定位仅为：

- 展示协议如何使用。
- 支持集成和截图测试。
- 帮助主题作者验证运行环境。

在外部主题目录、配置解析和故障回退完成前，可执行宿主把该 minimal theme 作为只读资源嵌入
二进制，使真实 greetd 纵向链路具备最小可操作界面。它仍是示例而不是固定产品 UI，并遵守：

- 不使用前端框架、包管理器、构建步骤、内联脚本或网络资源，只包含 allowlist 中的 HTML、
  CSS 和 JavaScript。
- 启动时调用 `state.get`，展示可信 session 摘要并保持 host 给出的默认选择；用户改变选择时
  只发送 `session.select`。
- 展示 `state.get` 中经过过滤的用户摘要和可选头像；选择摘要只填充其用户名，仍保留明确的
  “其他用户”手工输入路径，头像加载失败使用主题自身的非个人化 fallback。
- 一个表单先收集手工用户名，随后根据任意数量的 `auth.prompt` 动态切换 secret/visible
  输入。页面不假定 prompt 是密码，也不限制 PAM 轮数。
- 每次提交认证回答都先从 DOM 读取值，立即清空输入框并释放页面侧引用，再等待
  `auth.respond`；页面仍不声称 JavaScript 字符串可以被可靠清零。
- bridge 请求串行发送。等待响应期间禁用表单与 session 选择，并显示 busy 状态；
  `auth.message`、`auth.failed`、`auth.cancelled`、协议错误和 bridge 失败都以文本方式展示，
  不把消息作为 HTML 插入。
- 使用原生 label、form、input、select、button、`aria-live` 和键盘提交提供最小无障碍能力。
  greetd 返回认证错误后，Core 必须先发送 `CancelSession` 并确认旧会话释放，再向前端发布失败
  状态；登录失败或取消后恢复用户名输入，session 启动成功由 host 退出，不由页面导航处理。

该嵌入式主题已在真实 WebKitGTK/Wayland 实例中验证：allowlist 依次加载 HTML、CSS 和外部
JavaScript，脚本初始化后通过正式 bridge 发出 `state.get`；资源不需要网络、内联脚本或
宽松 CSP。认证与 session 行为继续由 controller 和真实 Unix socket stub 的全流程测试覆盖，
真实 PAM 输入则留给 greetd/Cage 系统测试，避免在开发会话中模拟用户密码。

### 8.1 React 参考主题

仓库在 `packages/fomalhaut-theme` 维护一个独立、私有且不参与 Semifold/npm 发布的官方参考
主题。它用于证明 `fomalhaut-sdk` 能支持完整的框架前端，并向主题作者提供可构建示例；它不
嵌入 Rust 二进制、不替代无构建依赖的内置 minimal theme，也不改变用户通过
`[frontend].path` 选择任意可信静态主题的能力。生产产物是 `dist/` 下的纯静态目录，根目录
包含 `theme.toml` 与 `index.html`，管理员可以直接让配置指向该绝对路径。

参考主题固定采用 React、TypeScript、Vite、Tailwind CSS v4、shadcn/ui Luma style 与
Zustand。依赖和脚本继续只由 Bun canary 管理；Vite 使用官方 React 与 Tailwind Vite plugin，
并设置 `base = "./"`，确保所有构建资源相对于 `fomalhaut://theme/` 加载。项目不引入 router、
SSR、服务端数据获取、CSS Modules、Sass、CSS-in-JS、远程字体或网络资源。shadcn 组件使用
CSS variables 和 Luma 的圆角、柔和层级与宽松布局基础。session 选择使用 shadcn/ui Luma
`Select`，不使用浏览器原生 `select`，避免 WebKit 与普通浏览器的 UA 样式差异。该组件允许
使用 Base UI 自身的 portal/positioner；浮层仍只能存在于当前可信主题文档中，不允许新窗口、
导航或放宽宿主 CSP。项目源码继续禁止手写 `style` prop 和内联 `<style>`。

所有项目自有文件与目录使用 ASCII `kebab-case`；`package.json`、`components.json`、
`tsconfig.json`、`index.html` 等生态固定单词文件名继续保持小写，Vite 配置显式命名为
`vite-config.ts`。TypeScript 类型和 React component 标识符仍使用语言惯例的 PascalCase。
项目添加文件名审计测试，阻止后续引入 PascalCase/camelCase 文件名。组件样式只使用 Tailwind
utility 与 shadcn semantic token，不允许 `style` prop、内联 `<style>` 或手写 component
selector；动态或较长的 `className` 必须通过 shadcn 提供的 `cn()` 分组组合。

前端只通过 workspace 中的 `fomalhaut-sdk` 访问宿主。SDK runtime 负责 client 生命周期与全部
v1 事件订阅，Zustand vanilla store 保存公开状态快照、选择、busy 和脱敏错误，并通过 React
provider 注入，便于 mock transport 测试。store 不使用 persist/devtools middleware，不写
localStorage/sessionStorage，不保存或记录 PAM 回答。认证输入使用不受控 DOM input：提交时先
读取值、同步清空 DOM 并释放页面侧引用，再调用 SDK；JavaScript 字符串无法可靠清零的限制
仍然成立。

主题是单文档、无 URL router 的内存 SPA。Zustand 使用判别状态在用户选择页、已知用户认证页、
其他用户认证页和身份未知的认证恢复页之间切换，不调用 history 或产生新的顶层导航。零个摘要
时入口页只显示“其他用户”；多个摘要时入口页将全部已知用户与“其他用户”作为一个整体水平
居中，用户显式点击已知摘要后才进入认证页。恰好一个可信摘要时跳过选择页，直接进入以大头像、
显示名、用户名和当前 session 为中心的已知用户认证页，并以其可信 `username` 调用
`auth.begin`。多个用户的显式选择采用相同的认证页和 `auth.begin` 流程。

认证页返回动作固定相对 viewport 定位，不得位于具有 transform/zoom 动画的祖先中；页面切换
只使用不会改变 fixed containing block 的过渡，避免返回按钮在首帧从内容中心跳到屏幕角落。

“其他用户”只作为选择页动作，不在认证页伪装成带头像和显示名的虚构账号。进入后直接显示
标题为 “Sign in” 的标准手工登录表单，同时渲染用户名与认证凭据两个输入区域。两个区域统一
使用 shadcn/ui `InputGroup` 组合输入、图标和提交动作，不用绝对定位按钮拼接输入框。受
greetd/PAM 顺序约束，初始只有用户名输入可用，凭据区域保持完整外观但禁用；用户确认用户名并
完成 `auth.begin` 后，收到的 `auth.prompt` 决定第二个输入是 secret 还是 visible，后续任意轮
prompt 在原位置替换。主题不得为了让两个输入同时可编辑而跨 `auth.begin` 暂存密码。认证输入
继续在请求前同步清空。
返回用户列表时，如果认证已开始，必须先成功执行 `auth.cancel`；取消失败则停留在认证页并
显示脱敏错误，避免在宿主仍有活动会话时开始另一用户认证。

页面运行期保留已选择或手工输入的用户名，但不持久化。刷新后若 `state.get` 表明认证仍在进行，
当前协议又没有提供活动用户名，主题进入不展示头像或猜测用户名的通用认证恢复页：有 prompt
时允许继续回答，同时始终允许取消并返回用户选择页。认证失败后可以保留当前运行期用户名以便
重试，但不得保留 PAM 回答。头像只使用 host 提供的不透明 `avatarUrl`，加载失败显示无个人
信息的 fallback。

视觉结构采用设备登录界面而非网页 Card：深空全屏背景使用 `#050812`、`#0A1730`、
`#102A52` 三层夜空底色，以 `#8EC5FF` 为交互冰蓝、`#F4F7FC` 为星光白、`#F2D6A2` 为少量
暖星高光。时间与日期位于左上，选择页主体居中，已知用户认证页使用 96px 头像与单一玻璃输入，
用户切换入口位于主体列表，session 控件固定在右下设备区。界面不使用居中的网页 Card、Toast
或大面积按钮容器；错误与 PAM message 在认证输入附近原位显示。视图切换和 focus 使用克制的
160–240ms 过渡，并遵守 `prefers-reduced-motion`。

普通浏览器中的 Vite 开发服务器没有 WebKit bridge，因此项目提供只在
`import.meta.env.DEV` 分支动态加载的 `development-transport.ts`，以实现
`FomalhautTransport` 并模拟公开状态、prompt、失败、取消和事件。它只是主题开发 fixture，
不等同于宿主级 demo mode。生产构建必须 dead-code eliminate 该 transport；缺少真实 bridge
时显示拒绝式错误，不能静默使用模拟认证。项目自有源码禁止调用 `fetch`、WebSocket 或其他
网络 API；构建测试检查产物没有 demo 标记，检查 HTML/CSS 没有远程 URL、inline
script/style、form navigation 或绝对资源 URL，并确认所有资源小于宿主 8 MiB 上限且清单位于
产物根目录。生产 JavaScript bundle 不采用简单的 `fetch(` 字符串禁令，因为 ReactDOM 19
自身包含 stylesheet preload 的内部 `fetch` 实现；它不是主题发起网络访问的授权边界。网络
隔离仍由主题源码审查、静态资源引用检查以及宿主 CSP/WebKit policy 共同强制执行。

测试至少覆盖 store 初始恢复和事件转换、零/多用户选择页、单用户跳过选择页并启动 PAM、居中
用户集合、已知用户与其他用户分支、身份未知的活动认证恢复、session 选择、secret/visible
多轮 prompt、回答在异步请求完成前已从 DOM 清空、busy 背压、取消失败不离开认证页、头像
fallback、文件命名和生产构建契约。CI 通过 Bun 运行 Biome、TypeScript、Vitest 和 Vite build；
最终还必须在 WebKitGTK 自定义 scheme 中验证 module script、CSS 与分块资源加载。

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

应用侧最初使用内置探针页面验证宿主能力；完成真实 core 和可信 session 接入后，该资源已
演进为上一节定义的嵌入式 minimal theme。当前仍不读取管理员主题目录，但已经连接真实
greetd，并继续维持以下已经验证的宿主边界：

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

嵌入式 minimal theme 只为首个可操作登录和协议示例提供基线；外部主题目录、配置、清单检查
和内置故障页面仍属于后续主题资源任务，不能通过继续扩展嵌入常量来替代。

在 Arch Linux、WebKitGTK 2.52.5、GTK 4.22.4 与 Cage 0.3.1 上的原型验证得到以下运行边界：

- 临时运行时探针分别触发了 HTTPS 顶层导航、新窗口和下载，宿主的 policy、create 与
  download 回调均在资源离开 WebView 前拒绝请求。另以回环地址监听器验证远程 `fetch` 和
  图片资源：页面脚本执行探针期间没有建立连接，说明当前 CSP 在网络请求前生效。
- 页面 reload 会先触发旧页面上下文失效日志，再为新文档重新建立 bridge；精确终止测试
  WebKitWebProcess 后，宿主观测到 `Crashed` 并退出。正式 core 接入后必须在同一个上下文
  失效入口取消活动认证，不能让新页面继承旧请求权限。
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

2026 年 8 月已在真实设备上由用户验证完整链路：greetd 以 `greeter` 用户通过 Cage 启动
Fomalhaut，内嵌 minimal theme 完成 PAM 交互并选择已发现的 Wayland session；
`StartSession` 成功后 Fomalhaut 与 Cage 正常退出，greetd 接管用户 session。该结果确认当前
纵向链路可用，但不替代后续自动化 Cage 回归、X11、失败恢复和更多发行版验证。

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

首个配置纵向切片固定读取 `/etc/fomalhaut/config.toml`，不接受前端、主题或普通进程环境变量
覆盖配置路径。文件缺失使用安全默认值；存在但不可读取或无效时退出。TOML 顶层和各 section
均拒绝未知字段，语法层只反序列化原始值，语义层再验证绝对路径、空值、数量与跨字段约束。
初始公开结构为：

```toml
[frontend]
path = "/etc/fomalhaut/themes/my-theme"

[sessions]
wayland_dirs = ["/usr/local/share/wayland-sessions", "/usr/share/wayland-sessions"]
x11_dirs = ["/usr/local/share/xsessions", "/usr/share/xsessions"]
executable_search_paths = ["/usr/local/bin", "/usr/bin"]

[power]
actions = ["poweroff", "reboot", "suspend"]

[display]
scale = 1.5
```

- `frontend` 缺失时选择内嵌 minimal theme；存在时只包含绝对主题目录，入口和协议版本由目录
  内必需的 `theme.toml` 决定，避免配置与清单出现两个互相冲突的入口来源。
- `sessions` 缺失时沿用固定默认目录。section 存在时，每个缺失字段仍继承对应默认值；显式
  空数组用于禁用该类目录。所有目录必须是无 NUL 的绝对路径，保持数组顺序作为优先级；
  至少要发现一个最终可用 session，否则启动失败。
- `power` 缺失时所有电源动作关闭。`actions` 是至多三个互不重复的枚举 allowlist，只接受
  `poweroff`、`reboot` 和 `suspend`；显式空数组等同关闭。配置顺序不影响 capability 的稳定
  顺序，宿主固定按 poweroff、reboot、suspend 排列，并与 logind 当前返回 `yes` 的动作求交集。
- `display` 缺失时页面缩放倍率为 `1.0`。`scale` 是应用于 WebKit `zoom-level` 的有限浮点数，
  允许范围为 `0.5..=4.0`；它缩放整个主题页面内容并支持小数倍率，不负责 Cage 光标大小，
  也不尝试从不可靠的 EDID 物理尺寸自动推断 DPI。管理员应按照 greeter 所在输出显式配置，
  例如与桌面环境的 `1.5` 倍缩放保持一致。
- 首个切片不加入可配置网络、CSP、开发者工具或任意 header。安全策略仍是编译期拒绝式常量，
  避免把主题配置扩展成降低宿主边界的权限开关。
- 日志目标和记忆用户/session 继续留作后续字段；在实现前未知字段会被拒绝，不能
  提前依赖未承诺的配置键。

配置与外部主题纵向切片已用自动化测试验证：配置缺失时安全回退、未知字段和相对路径拒绝、
显示缩放边界、显式 session 优先级、64 KiB 上限、清单 protocol/入口校验、URI 语法、MIME 白名单、顶层导航
限制、配置根 symlink、根内相对 symlink、根外 symlink 拒绝以及资源读取边界。完整 workspace
测试同时继续覆盖真实 Unix socket greetd 流程；内嵌主题仍通过 Wayland/WebKitGTK 运行探针
验证，外部主题的真实系统安装步骤记录在 `docs/CONFIGURATION.md`。

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
- `fomalhaut-sdk` 作为独立 Node.js package 维护版本，由 Semifold Node.js resolver 和英文
  changeset 决定提升级别；本地同样不得执行 `smif version` 或 `smif publish`。
- 所有 crate 使用 Rust 2024 Edition，并跟随最新 Rust stable，不承诺固定 MSRV。
- Cargo manifest 不设置 `rust-version`；CI 不维护旧 Rust 版本兼容性矩阵。
- Rust stable 或依赖升级引起的必要技术变动，仍须先更新本文和 `TODO.md` 再实施。
- 第三方依赖跟随最新稳定版本，但通过 manifest 语义化约束和已提交 lockfile 保持构建可复现。
- 前端协议单独维护整数主版本。
- 在所有 package 仍处于 alpha release channel 期间，不承诺前端协议或 JSON Schema 向后
  兼容；允许为了最佳结构直接修改同一协议主版本的字段、请求和约束，并同步 Rust、Schema、
  SDK、内置主题与 changeset。进入 beta/rc/稳定通道前必须重新定义兼容承诺，此后不得沿用
  该 alpha 例外。
- 只修改手写 SDK Client 时只提升 `fomalhaut-sdk`；Rust wire 类型变化并改变生成产物时，
  changeset 必须同时包含 `fomalhaut-web` 和 `fomalhaut-sdk`。
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
- session desktop entry 基本格式使用 `freedesktop-desktop-entry`，登录 session 的严格
  `Exec` 校验和安全策略由 `fomalhaut-session` 实现。
- WebKitGTK、Cage 和 greetd 的最低兼容版本；Rust 工具链继续跟随 stable。当前开发依赖
  跟随滚动最新稳定版本，最低兼容版本只能在发行版验证后声明。

这些决策不得削弱本文定义的 core/UI 分离和前端权限边界。
