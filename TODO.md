# Fomalhaut TODO

本清单按依赖关系和风险排序。`P0` 是构建可测试最小系统的阻塞项；`P1` 完成首个可用
greeter；`P2` 用于加固和发行；`P3` 是后续增强。

## P0：项目基础与无 UI 核心

### 仓库和 workspace

- [x] 初始化 Git 仓库并添加 `AGPL-3.0-only` 许可证。
- [x] 创建使用 resolver 3 和 `crates/*` 成员模式的虚拟 Rust workspace。
- [x] 创建 `fomalhaut-core`、`fomalhaut-session`、`fomalhaut-web` 和 `fomalhaut`
      四个 crate。
- [x] 配置 Rust 2024 Edition、滚动 stable 工具链、rustfmt、Clippy 和测试命令。
- [x] 将四个 crate 设置为独立的 `0.1.0-alpha` 版本并初始化 Semifold Rust resolver。
- [x] 将全部包的 Semifold release channel 设置为 `alpha`，并限制 version/publish 只能
      由 GitHub Actions 执行。
- [x] 统一使用英文编写 Semifold changeset。
- [x] 添加基础 CI：格式检查、Clippy、单元测试和文档构建。
- [x] 将基础 CI job 缩短为 `Checks`，并为 Rust 构建与 Bun package 下载配置 lockfile-aware
      cache；Bun 依赖仍由 frozen lockfile 重建。
- [x] 编写贡献说明和基础 README。

### Core 状态机

- [x] 定义 `GreeterState`、`GreeterEvent`、`PromptKind`、`PromptId` 和结构化错误。
- [x] 定义不泄漏内容并在 drop 时清零的 `Secret` 类型。
- [x] 定义 transport 抽象，使 core 可以连接真实 Unix socket 或测试 stub。
- [x] 使用 `greetd_ipc` 实现 Unix socket transport。
- [x] 实现 `create_session`。
- [x] 实现 secret 和 visible prompt 处理。
- [x] 使用 `PostAuthMessageResponse { response: None }` 自动确认 info/error PAM 消息。
- [x] 实现带 `PromptId` 校验的 `respond`。
- [x] 实现 `cancel`。
- [x] 实现只允许在认证成功后调用的 `start_session`。
- [x] 保证同一时刻只有一个 greetd 请求等待响应。
- [x] 处理连接断开、greetd error 和不可恢复协议错误。
- [x] 正常退出显式等待取消；Drop 清理敏感内存并关闭 transport，不执行异步 IPC。

### Core 测试

- [x] 覆盖所有合法状态转换。
- [x] 覆盖非法、重复和乱序操作。
- [x] 测试过期及重复 `PromptId`。
- [x] 测试用户名 + 密码流程。
- [x] 测试多轮 MFA 流程。
- [x] 测试 visible、secret、info 和 error 的混合流程。
- [x] 测试认证失败后重新认证。
- [x] 复核错误密码的 `AuthError → CancelSession → Failed → auth.begin` 恢复链路，以及活动取消、
      已释放失败会话返回和 PAM error 消息重试清理行为。
- [x] 测试无密码账户。
- [x] 测试 session 启动成功和失败。
- [x] 测试 socket 断开及主动取消。
- [x] 验证日志、`Debug` 和错误中不包含 secret。
- [x] 集成或编写 greetd stub 测试后端。

## P0：前端协议和可信 session

### 前端协议 v1

- [x] 定义请求、响应、事件和错误的公共 envelope。
- [x] 定义 `state.get`。
- [x] 定义 `auth.begin`、`auth.respond` 和 `auth.cancel`。
- [x] 定义 `session.select`。
- [x] 定义 `power.request`，但在策略层完成前保持禁用。
- [x] 为事件增加单调递增 sequence。
- [x] 为请求增加唯一 ID 和对应响应。
- [x] 规定字段与消息的最大长度，并用 Schema 扩展注解准确表达 UTF-8 byte 上限。
- [x] 对未知字段、未知方法、版本、JavaScript safe integer 和 UTF-8 byte 长度进行严格校验。
- [x] 为认证回答实现 zeroizing、脱敏的 wire 类型并直接转换为 core `Secret`。
- [x] 创建 `protocol/v1.schema.json`。
- [x] 添加 JSON schema 与 Rust 序列化类型的一致性测试。
- [x] 记录协议兼容和版本升级规则。

### Session discovery

- [x] 定义不透明且可稳定重建的 `SessionId`。
- [x] 定义有序、显式标注 X11/Wayland 类型的搜索目录和逐项拒绝诊断。
- [x] 禁用 desktop entry parser 的 gettext feature，仅使用文件内本地化字段。
- [x] 解析 Wayland desktop session。
- [x] 解析 X11 desktop session。
- [x] 处理 `Hidden`、`NoDisplay`、无效布尔值、无效 `TryExec`、无效 `Exec` 和重复项。
- [x] 兼容省略 `Type` 的 Plasma session desktop entry，同时继续拒绝显式非 Application 类型。
- [x] 严格解析 `Exec` argv，拒绝除 `%%` 外的 field code，且不经 shell 执行。
- [x] 配置 session 搜索目录及优先级。
- [x] 将 session ID 映射为只在 Rust 内部可见的 `SessionCommand`。
- [x] 确保前端不能覆盖 executable、arguments 或 environment。
- [x] 添加 desktop entry fixture 和安全边界测试。

## P0：WebView 技术原型

- [x] 选择原生 GTK4 + WebKitGTK 6.0 宿主，不使用 Tao、Wry、Tauri 或 WPE。
- [x] 启用 `webkit6/gtk_v4_18` 并验证 GTK 4.18+ 编译基线。
- [x] 在 `fomalhaut` 中直接使用 `gtk4`/`webkit6` 制作最小全屏原型。
- [x] 验证 Wayland 和 Cage 下的启动。
- [x] 通过 `fomalhaut://theme/` 自定义 scheme 加载带固定 MIME、安全 header 和仅允许
      `fomalhaut:` 静态资源的 CSP。
- [x] 记录并测试 WebKitGTK 2.52 自定义 scheme 与 `nosniff` 的外部脚本兼容性例外，保证
      MIME 仍由 Rust 白名单固定且不按主题输入推测。
- [x] 将自定义 scheme 仅注册为 secure 和 display-isolated，不启用 CORS/local/no-access，
      并测试其精确资源白名单及 CSP 边界。
- [x] 通过单一 WebKit script message handler 验证 JavaScript 到 Rust 的协议 v1 bridge。
- [x] 验证 Rust 到 JavaScript 只投递序列化后的协议消息。
- [x] 验证导航、新窗口和下载拦截。
- [x] 验证默认禁止远程网络资源。
- [x] 默认关闭开发者工具、自动弹窗和非必要 Web 能力。
- [x] 让 renderer 终止、页面刷新和窗口退出进入可观察的拒绝式处理路径。
- [x] 调查并记录 renderer sandbox 行为。
- [x] 记录 Arch Linux 上 GTK4/WebKitGTK/Cage 的运行时依赖、包体积和调试构建 RSS 快照。
- [ ] 测量发布构建的 PSS/峰值，并记录非 Arch 发行版的包名、可用版本与打包成本。

## P1：首个可用 greeter

### Host 集成

- [x] 在 `fomalhaut-web` 实现不依赖 GTK 的认证 controller，维护公开状态、core prompt 映射
      和单调事件 sequence。
- [x] 在 GTK 主线程和专用单线程 Tokio worker 之间建立双向有界通道及单请求背压。
- [x] 为 bridge 请求和 worker 输出增加页面 epoch，拒绝刷新前的旧 reply 和事件。
- [x] 从 `GREETD_SOCK` 建立 core 连接。
- [x] 把 core event 转换成公开状态和有序前端事件。
- [x] 把 `state.get`、`auth.begin`、`auth.respond` 和 `auth.cancel` 转换成经过状态检查的 core
      调用，并继续禁用 power 请求。
- [x] 使用不受环境变量影响的默认目录发现 session，拒绝空 catalog 和目录级错误。
- [x] 将 catalog 转换为 controller 内部的公开摘要与可信 `SessionCommand`，默认选择稳定
      顺序中的第一项。
- [x] 实现只接受 catalog 内不透明 ID 的 `session.select`，并发出有序选择事件。
- [x] 实现 session 选择并在认证成功后启动可信 session。
- [x] 页面刷新或 bridge 断开时取消活动认证。
- [x] 处理 WebView renderer 崩溃。
- [x] 正常关闭时显式取消活动认证并等待 controller worker 退出。
- [x] 使用 greetd stub 覆盖 bridge/controller 的密码、失败、过期 prompt、取消与刷新流程。
- [x] 登录成功后正确退出 Fomalhaut。
- [x] 确保 Fomalhaut 退出后 Cage 能退出并让 greetd 接管用户 session。
- [x] 使用真实 DM 环境完成 Cage 退出与 greetd 接管的端到端验证。

### 配置

- [x] 定义拒绝未知字段、限制 64 KiB 的 `/etc/fomalhaut/config.toml` 配置模型。
- [x] 分离 TOML 语法解析和绝对路径、空值及跨字段约束的语义验证。
- [x] 支持外部主题目录配置；入口和协议版本由必需的 `theme.toml` 提供。
- [x] 支持 Wayland/X11 session 目录及 `TryExec` 搜索目录配置，并保持安全默认值。
- [ ] 支持日志级别和日志目标。
- [ ] 支持是否记住上次用户和上次 session。
- [ ] 对安全相关配置提供拒绝式默认值。
- [x] 为配置添加示例、schema 或完整字段文档。

### 主题资源加载

- [x] 让现有 `fomalhaut://theme/` scheme 在内嵌主题与外部 capability 目录间统一调度。
- [x] 实现清单入口文件和最大 8 MiB 的静态资源加载。
- [x] 实现固定 MIME 类型白名单。
- [x] 严格拒绝 `..`、`.`、空 segment、反斜杠、百分号、query 和 fragment。
- [x] 使用目录 capability 和打开后的文件描述符读取资源；回归测试允许根内 symlink，并拒绝
      已知的根外 symlink escape。
- [x] 设置严格 CSP。
- [x] 默认禁止外部 URL、远程脚本和远程字体。
- [x] 实现最大 16 KiB、拒绝未知字段的主题清单及前端协议版本检查。
- [ ] 实现内置最小故障页面。

### Minimal theme

- [x] 将内置 bridge probe 升级为连接真实 controller 的只读嵌入式 minimal theme，作为外部
      主题加载完成前的可操作登录基线。
- [x] 提供无框架、无构建依赖的 minimal theme。
- [x] 支持手工输入用户名。
- [x] 支持 session 选择。
- [x] 动态支持 secret 和 visible prompt。
- [x] 支持多轮 PAM prompt。
- [x] 展示 info、error、busy 和失败状态。
- [x] 提交后立即清空输入框并释放 secret 引用。
- [x] 支持键盘操作和基础无障碍标签。
- [x] 明确标注该主题仅为示例，不是固定产品 UI。

### greetd/Cage 集成

- [x] 编写 greetd 配置示例。
- [x] 编写 Cage 启动示例。
- [x] 验证低权限 `greeter` 用户运行。
- [ ] 验证 VT 切换和失败恢复。
- [x] 验证 Wayland session 登录。
- [ ] 验证至少一种 X11 session 登录方案。
- [ ] 记录 compositor 和系统运行时依赖。

## P1：TypeScript SDK

### Workspace 与发布

- [x] 初始化 Bun workspace 和无 scope 的 `fomalhaut-sdk` package，保持纯 ESM、零运行时
      依赖并独立使用 `0.1.0-alpha` 初始版本。
- [x] 通过 `smif config sync --resolver nodejs` 注册 package，并将其 release channel 设为
      `alpha`；不得手工模拟 Semifold 配置或在本地执行 version/publish。
- [x] 保留 Semifold CI 的 `npm publish --provenance --access public` 作为 OIDC 发布专用例外；
      npm 不得参与安装、workspace、脚本、测试、构建或 lockfile，根 private package 不得进入
      Semifold 发布列表。
- [x] `fomalhaut-sdk` 首次发布后移除 npm token、registry auth 和 `.npmrc` 配置；后续由
      Node.js 24/npm 运行时与 GitHub `id-token: write` 执行 OIDC-only 发布。
- [x] 只使用 Bun 管理 Node workspace，提交文本格式 `bun.lock`，并拒绝 npm、pnpm 或 Yarn
      lockfile；CI 使用 `bun install --frozen-lockfile`。
- [x] 本地使用 `bun upgrade --canary`，GitHub Actions 使用 `oven-sh/setup-bun@v2` 和显式
      `bun-version: canary`；不得填写尚未发布的 `1.4.0` 或回退到 stable/latest。
- [x] 在 CI 输出 `bun --version` 和 `bun --revision`，记录每次滚动 canary 实际验证的 Rust
      实现提交。
- [x] 使用锁定的最新稳定 TypeScript 与 Biome，通过 `bun add` 管理并建立滚动更新策略。
- [x] 配置 package exports、declaration 输出和只包含发布产物的 npm files 白名单。

### Rust 类型生成

- [x] 通过 Cargo CLI 为 `fomalhaut-web` 添加 `ts-rs`，为所有公开 wire 类型派生 TypeScript。
- [x] 使用 `#[ts(export, export_to = "...")]` 按 Rust 协议模块聚合导出
      `protocol-error.ts`、`protocol-request.ts`、`protocol-message.ts` 和
      `protocol-secret.ts`，保证所有 TypeScript 和 binding 文件名只使用 ASCII
      `kebab-case`。
- [x] 让普通测试导出到 `target/ts-rs`，仅由显式生成命令写入
      `packages/fomalhaut-sdk/src/generated`，不得使用 `build.rs` 或 greeter 启动流程生成。
- [x] 将 JavaScript-safe 的 `RequestId`、`PromptId` 和 `Sequence` 精确映射为 `number`，并验证
      `EmptyParams` 不会退化成宽泛的 `{}`。
- [x] 生成后运行 Biome format 与只读 check，并以“重新生成后 Git 无 diff”测试 Rust、JSON
      Schema 和 TypeScript 产物一致性。
- [x] 为不能由 TypeScript 表达的 UTF-8 byte 和集合上限保留 Schema/Rust 权威说明。
- [ ] 为不接受 `application/schema+json` 或无法联网的 IDE 提供 Draft 2020-12 schema catalog、
      缓存或本地映射指引，不得把 Schema dialect 改写为 Draft-07。

### Client API

- [x] 定义可注入的 `FomalhautTransport` 和默认 `WebKitTransport`。
- [x] 实现 `state.get`、`session.select`、`auth.begin`、`auth.respond` 和 `auth.cancel` 的强类型
      Client API；power policy 完成前不提供高级电源方法。
- [x] 实现按事件名收窄的 typed event subscription 和取消订阅。
- [x] 由 Client 管理 JavaScript-safe request ID，并校验响应 ID、协议版本和单调 event
      sequence。
- [x] 区分协议、bridge 和本地 busy 错误；同一时刻只允许一个请求，不排队或记录认证回答。
- [x] 使用 mock transport 覆盖成功、协议拒绝、bridge 失败、并发、乱序和重复事件。
- [x] 添加由 `bun run` 调度的 Biome CI、TypeScript source/test typecheck、build 和生成产物
      漂移检查，并使用 `bun test` 运行 SDK 单元测试；测试目录使用就近 `tsconfig.json` 加载
      `bun:test` 类型并纳入 IDE configured project。
- [x] 编写 `fomalhaut-sdk` 快速入门。
- [ ] 让后续 minimal theme 构建版本使用 SDK 而非手写 bridge 调用。

## P1：开发体验

- [ ] 实现不访问真实 greetd 的 demo mode。
- [ ] 在 demo mode 模拟密码、MFA、visible prompt、失败和取消。
- [ ] 在 demo mode 禁用真实 session 和电源操作。
- [ ] 在 UI 中明确标识 demo mode。
- [ ] 提供主题开发命令和热重载方案评估。
- [ ] 编写前端协议快速入门。
- [ ] 编写从纯 HTML 到框架构建产物的主题示例说明。

## P1：React 参考主题

- [x] 在 `packages/fomalhaut-theme` 初始化不参与发布的 Bun/Vite React TypeScript 项目，使用
      Tailwind CSS v4 Vite plugin、shadcn/ui Luma style、Zustand、Biome 与 Vitest。
- [x] 保证所有项目自有文件/目录为 ASCII kebab-case，添加命名审计；长或动态 `className`
      使用 shadcn `cn()`，不引入 inline style、CSS Modules、CSS-in-JS 或远程资源。
- [x] 以可注入 `FomalhautClient`、runtime、Zustand vanilla store 和 React provider 实现
      `state.get`、全部 v1 事件、busy 背压、脱敏错误恢复以及选择/已知用户/其他用户/通用恢复
      SPA 状态，不持久化或记录 PAM 回答。
- [x] 实现 Nocturne 设备登录界面：零/多用户使用整体居中的选择页，单用户跳过选择页并直接
      启动 PAM；已知用户进入大头像认证页；“其他用户”进入无虚构头像的 Sign in 表单，并以
      shadcn InputGroup 同时呈现用户名与按顺序启用的 prompt；取消成功后才能返回；同时覆盖
      时间日期、可信 session、头像 fallback、任意轮 prompt 和刷新恢复。
- [x] 实现仅 Vite DEV 动态加载的 `development-transport.ts`，生产缺少 WebKit bridge 时拒绝；
      源码审计禁止网络 API，构建检查不得包含 demo 标记、远程资源引用、inline script/style
      或越界资源；不对包含 ReactDOM 内部 stylesheet preload 的生产 bundle 使用字符串级
      `fetch(` 禁令。
- [x] 让 `dist/` 包含相对资源引用、`index.html` 和 `theme.toml`，补齐 store、组件、DOM 清空、
      文件命名与构建契约测试，并把 format/typecheck/test/build 接入根脚本和 CI。
- [ ] 在真实 WebKitGTK/Cage 中验证 React module script、CSS、用户/session、认证与退出流程。

## P1：用户发现与头像

- [x] 在严格配置中加入 `auto`、`accounts_service`、`nss`、`none` 用户 provider，默认
      AccountsService，只有整体不可用时回退 NSS；发现失败或超时不得阻止手工登录。
- [x] 实现内部可替换 provider trait、AccountsService 只读 D-Bus provider 与固定执行
      `/usr/bin/getent passwd` 的受限 NSS fallback；`AccessDenied`、成功空列表和逐项失败不得
      触发 fallback，并覆盖超时、超限、失败、重复、越界与确定性排序测试。
- [x] 为协议 `StateSnapshot` 增加最多 128 个 `UserSummary`，同步 Draft 2020-12 Schema、
      ts-rs 生成类型、SDK 测试和所有 Rust controller 构造路径；alpha 阶段允许直接调整 v1。
- [x] 实现不暴露原始路径的头像代理：nofollow 打开、文件所有权/AccountsService 根检查、
      2 MiB 上限、PNG/JPEG/WebP magic allowlist、内存资源和精确
      `fomalhaut://avatar/<id>` handler。
- [x] 更新内置 minimal theme：展示用户、显示名和头像，同时始终保留“其他用户”手工输入与
      头像失败 fallback。
- [x] 更新配置、协议和主题作者文档；AUR 将 `accountsservice` 声明为显示名和头像的可选增强
      依赖。
- [ ] 在真实 greeter 环境验证 AccountsService 可用/不可用、NSS fallback、禁用枚举和无头像
      场景。

## P2：安全加固

- [ ] 编写正式 threat model。
- [ ] 对 bridge 的每个方法实施白名单和状态检查。
- [ ] 对所有前端字符串、ID 和消息实施长度限制。
- [ ] 检查日志、panic 和错误链中的敏感信息泄漏。
- [ ] 禁用开发者工具、下载、弹窗、任意导航和不需要的 Web API。
- [ ] 验证正式模式没有 TCP 监听端口。
- [ ] 审计主题路径规范化和资源 scheme。
- [ ] 独立审计 `cap-std` 目录 capability 的 symlink 与竞态保证，并保留根内允许、根外拒绝
      的回归测试。
- [ ] 为“外部主题必须由管理员信任并审查”的当前安全前提编写运维警告，并评估主题打包、
      签名、来源验证或更强隔离机制；不得把 capability/CSP 描述为对恶意主题代码的完整沙箱。
- [ ] 审计 desktop entry 到 `SessionCommand` 的转换。
- [x] 实现默认关闭的电源 allowlist、systemd-logind `Can*` 能力交集和非交互
      `PowerOff/Reboot/Suspend` 执行；认证中请求须先取消 greetd 会话，且不得回退到 shell。
- [x] 在 SDK 暴露类型化 `power.request`，并在 Nocturne 主题只按 capability 渲染带确认步骤的
      电源菜单。
- [x] 覆盖配置校验、controller 策略/取消顺序、logind 能力映射、SDK 请求和主题交互测试。
- [ ] 在真实 greeter 环境验证 poweroff、reboot、suspend、Polkit `challenge`、inhibitor 和
      logind 不可用时的安全退化。
- [ ] 评估剪贴板、拖放、文件选择器和自定义 URL handler。
- [ ] 记录 JavaScript/WebView 无法保证 secret 清零的限制。
- [ ] 在目标发行版验证 WebKit renderer sandbox。
- [ ] 对协议解析和状态机增加 fuzz/property tests。

## P2：可靠性与可维护性

- [x] 修复 greetd `AuthError` 后显式取消旧会话再发布失败状态，并覆盖取消顺序及重试测试。
- [x] 修复参考主题认证页返回按钮的 transform containing-block 首帧跳动。
- [x] 将参考主题 session 控件替换为 shadcn/ui Luma `Select`，覆盖选择交互和构建产物测试。
- [ ] 添加结构化 tracing，定义敏感字段过滤策略。
- [ ] 实现明确的进程退出码。
- [ ] 为不可恢复错误提供故障页面和诊断指引。
- [ ] 添加 WebView 生命周期及页面刷新集成测试。
- [ ] 添加 Cage 下的端到端登录测试。
- [ ] 测试 host 被终止时 greetd session 的清理行为。
- [ ] 测试主题缺失、损坏和协议版本不匹配。
- [ ] 测试多显示器和高 DPI 的基本行为。
- [x] 实现严格的 `[display].scale` 配置并应用 WebKit 页面 zoom，覆盖默认值、小数倍率和非法
      浮点/边界测试；光标缩放继续由 Cage 管理。
- [ ] 测试非 ASCII 用户名、prompt 和 session 名称。
- [x] 建立依赖和 Rust stable 滚动更新策略。

## P2：打包与首个发行版

- [x] 添加根目录安全安装器：支持首次/更新构建、原子二进制与主题部署、TOML 备份验证更新、
      显式 greetd restart 和临时 system-root 集成测试。
- [x] 让源码安装器在 Arch 上检查构建与运行包，按 `paru`、`yay`、`sudo pacman` 优先级安装
      缺失的系统依赖；Rust 与 Bun 工具链由用户自行提供。
- [x] 让源码安装器达到内容级幂等：连续相同安装不新增二进制/配置备份或主题 release。
- [x] 让源码安装器首次创建 Fomalhaut 配置时默认允许 poweroff、reboot 和 suspend，并验证原地
      更新保留缺失、显式关闭或自定义的既有电源策略。
- [x] 为源码安装器添加遵守 TTY 与 `NO_COLOR` 的分级彩色输出。
- [ ] 确定 greetd、WebKit 和 Cage 的最低版本；Rust 继续跟随 stable。
- [ ] 提供 systemd-tmpfiles 配置（如果需要状态或日志目录）。
- [ ] 提供示例 greetd 配置和安装说明。
- [ ] 提供 shell completions 和 man page（如 CLI 稳定）。
- [ ] 准备 Nix 或其他后续目标发行版的打包方案。
- [x] 添加 `greetd-fomalhaut` AUR 源码包模板：将上游预发布 SemVer 映射为合法 Arch
      `pkgver`，以 `pkgrel` 表达纯打包修订，并把 greetd、Cage、D-Bus、GTK4 和 WebKitGTK
      6.0 声明为必需运行时依赖；按 ELF `NEEDED` 显式声明当前 Arch 的直接 ABI 提供者。
- [x] 让 AUR 包安装 `/usr/bin/fomalhaut`、AGPL-3.0-only 许可证、配置文档和不覆盖系统配置的
      greetd/Cage 示例；AUR 打包元数据单独使用 0BSD，并明确其不重新许可上游软件。
- [x] 添加由成功 `Semifold CI` 和手动调度触发的 AUR workflow，只跟踪
      `fomalhaut-v*`：校验 crates.io 可见性、避免重复版本、计算源码 SHA-256，并在干净 Arch
      环境执行 frozen 构建、测试、`.SRCINFO` 生成和 `namcap`；自动发布不得降低已有
      `pkgrel`，手动打包修订只接受严格递增值。
- [x] 使用受保护的 `aur-production` Environment、人工审批和专用 SSH key 推送独立
      `greetd-fomalhaut` AUR Git 仓库。
- [ ] 在 GitHub 配置 AUR maintainer variables、`AUR_SSH_PRIVATE_KEY` secret 和
      `aur-production` 审批规则；最终应用标签产生后完成首次 clean Arch build、`namcap`、
      自动扫描并核验固定的 AUR Ed25519 主机指纹、人工审批和 AUR 推送验证。
- [ ] 编写升级和前端协议兼容说明。
- [x] 使用 Semifold 生成的 status 与 CI workflow，由 GitHub Actions 独占 version 和
      publish。
- [x] 将 status 与 CI workflow 的 Semifold CLI pin 到本地 `0.3.0-rc.1` 对应的 action 版本
      `v0.3.0-rc.1`。
- [x] 为四个 Rust crate 补齐 crates.io 所需 description、license 和 repository metadata，
      并为 `fomalhaut-sdk` 补齐 npm repository metadata。
- [x] 在 Semifold CI 发布前使用 frozen `bun.lock` 安装依赖并构建 SDK，确保 npm payload
      包含 `dist`。
- [x] 将 Semifold CI runner 与基础 CI 对齐到 `ubuntu-26.04` 并安装 WebKitGTK 开发包，使
      `cargo publish` 获得 GTK 4.18+ 并验证最终 greeter crate。
- [x] 在首次正式发布前对四个 crate 执行 `cargo package` payload 检查，并对 SDK 执行
      `npm pack --dry-run`。
- [ ] 下次切换 Semifold release channel 时，同步修改并审核 npm publish 的显式 dist-tag，
      不得沿用当前 `--tag alpha`。
- [ ] 完成许可证、第三方依赖和资源归属检查。
- [ ] 发布前执行完整安全检查和端到端测试。

## P3：后续增强

- [ ] 可配置的用户头像 provider。
- [ ] 记住上次用户和每用户上次 session。
- [ ] 本地化 core 消息和示例主题。
- [ ] 多显示器显示策略。
- [ ] 键盘布局选择。
- [ ] 无障碍增强和屏幕键盘集成。
- [ ] 可选的原生 UI host，复用 `fomalhaut-core`。
- [ ] 主题兼容性测试工具。
- [ ] 主题打包、签名或来源验证机制评估。
- [ ] 稳定的第三方 host API。

## 首个里程碑定义

### M0：Headless core

- core 能通过 stub 完成密码、多轮 MFA、失败、取消和 session 启动流程。
- 所有流程无需 WebView 或真实图形环境即可测试。
- secret 不出现在日志和 `Debug` 输出中。

### M1：WebView proof of concept

- 在 Cage 下加载本地 HTML。
- JS 可以使用版本化 bridge 驱动 stub core。
- 外部导航和网络默认被阻止。

### M2：可登录系统

- 使用真实 greetd 完成认证。
- 使用可信 `SessionId` 启动 Wayland session。
- 用户可以替换整个主题目录。
- 页面刷新、失败和退出不会遗留活动认证 session。

### M3：首个可发布版本

- 完成安全审计清单和端到端测试。
- 有明确的依赖、安装、配置、主题开发及故障排查文档。
- 前端协议 v1 和配置格式进入兼容性维护阶段。
