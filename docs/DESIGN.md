# Fomalhaut 技术设计

## 1. 项目概述

Fomalhaut（北落师门）是一套使用 Web 技术呈现本机认证界面的 Rust 项目。登录界面
（greeter）和会话锁屏（locker）是两个同等重要、独立交付的产品角色；两者共享认证领域
模型、前端协议、主题资源策略、TypeScript SDK 以及 GTK4 + WebKitGTK 6.0 宿主能力，但拥有
不同的可信后端和系统生命周期。

当前 greeter 继续以 [greetd](https://git.sr.ht/~kennylevinsen/greetd) 作为登录认证与用户
session 管理后端，通过 greetd IPC 驱动认证和 `StartSession`。locker 运行在已经登录的用户
Wayland session 中，通过 `ext-session-lock-v1` 持有 compositor 锁，并使用只允许当前 session
用户重新认证的 PAM backend；locker 不通过 greetd 启动或切换 session。

长期可以设计系统服务 `fomalhautd`，逐步承接登录认证、重新认证、session 监督、seat/VT、
logind 和电源策略，并最终具备替代 greetd 的能力。`fomalhautd` 目前只是明确的演进方向，
不是实现 locker 的前置条件；在其特权边界、IPC 和 session 恢复语义完成独立设计与审计前，
greeter 不移除 greetd backend。

本项目是独立实现。`tuigreet` 仍只作为 greetd IPC 行为、session 发现和测试方法的参考；
`swaylock` 用作 session-lock 生命周期、PAM 隔离和失败关闭语义的参考，二者都不作为 Fomalhaut
的源码基础。

## 2. 设计目标

### 2.1 核心目标

- 将共享认证状态机、Secret 和 backend 能力封装为不依赖 greetd、PAM 和具体 UI 的 Rust
  core。
- 将 greetd 登录和当前用户 PAM 重新认证实现为权能不同、可替换的 backend。
- 使用统一的 GTK4 + WebKitGTK 6.0 宿主渲染 greeter 和 locker。
- 使用 `ext-session-lock-v1` 实现失败关闭的 Wayland session lock，不以普通全屏窗口或
  layer-shell 模拟安全锁屏。
- 不固化前端框架、构建工具或视觉设计。
- 允许管理员为 greeter 和 locker 配置同一个通用主题，也允许分别配置两个单页面主题。
- 为前端提供稳定、版本化且最小化的消息协议。
- 在主题能够读取页面认证输入的既定边界下，继续限制其系统、文件、网络和进程权限。
- 正确支持 PAM 的多轮、任意类型认证提示，而非仅支持“用户名 + 密码”。
- 让 core、会话发现和协议转换能够在无图形环境中完成自动化测试。

### 2.2 次要目标

- 支持发现 X11 和 Wayland desktop session。
- 支持保存上次用户和上次会话等非敏感偏好。
- 提供关机、重启等经过配置和授权的系统操作。
- 提供便于主题开发的独立预览或演示模式。
- 提供一个同时覆盖 greeter/locker 的极简示例主题，用于协议演示和集成测试。
- 为将来接入 `fomalhautd` 保留 backend seam，但不提前扩大当前 daemon 权限面。

### 2.3 非目标

- 当前阶段不实现 `fomalhautd`，也不替代 greetd 的登录 session 管理。
- greeter 不直接调用 PAM；locker 的 PAM 使用范围只限当前 session 用户的重新认证。
- 不提供远程登录网页或监听局域网的登录服务。
- 不允许前端直接连接 greetd socket、PAM worker 或未来的 daemon IPC。
- 不允许前端提交任意可执行文件、命令行、环境变量或文件路径。
- 不承诺 JavaScript/WebView 内存中的密码能够像 Rust 缓冲区一样可靠清零。
- locker 首阶段只支持广告 `ext-session-lock-v1` 的 Wayland compositor；X11 锁屏和未实现该
  协议的桌面环境不在首阶段兼容范围。
- 不实现完整的窗口管理器或 Wayland compositor。
- 初期不提供通用浏览器能力，例如任意导航、下载、扩展或开发者工具。

## 3. 系统边界

```text
                              fomalhaut-core
                  认证领域类型 / Secret / backend traits
                         /                         \
                        ▼                           ▼
              fomalhaut-greetd                fomalhaut-pam
                 LoginBackend                 ReauthBackend
                        │                           │
                     greetd                    PAM worker
                        │                           │
                        ▼                           ▼
                fomalhaut greeter            fomalhaut-lock
                Cage/普通 GTK 窗口       ext-session-lock-v1 窗口
                         \                        /
                          ▼                      ▼
                    fomalhaut-web + fomalhaut-gtk
                   协议 / 主题 / GTK4 / WebKitGTK
                                  │
                                  ▼
                         单页面可信 Web 主题
```

greeter 只有 `LoginBackend` 可以访问 greetd socket 和启动可信 session；locker 只有
`ReauthBackend` 可以访问受限 PAM worker，而且身份由当前 UID/session 推导，前端不能提交或
切换目标用户。locker 主进程独占 `ext-session-lock-v1` handle；PAM backend 和 Web 前端只能
产生内部的“允许解锁”结果，不能直接调用 Wayland unlock。

主题是管理员选择的可信代码，可以读取用户在页面中输入的认证回答；但它不能获得 greetd、
PAM worker、未来 daemon IPC、原始文件系统或任意进程执行能力。Fomalhaut 是本机认证界面，
不是 Web 服务器；正式运行时继续使用 WebView 自定义资源协议提供静态文件，不开放 TCP 端口。

未来 `fomalhautd` 应位于 backend 之后，向 greeter 和 locker 分别提供权能隔离的 login 与
reauthentication 接口；它不渲染界面，也不持有用户 compositor 的 session lock。

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
│   ├── fomalhaut-core/       # backend-neutral 认证领域核心
│   ├── fomalhaut-greetd/     # greetd login backend
│   ├── fomalhaut-pam/        # 当前用户 reauth backend
│   ├── fomalhaut-session/    # 可信 session discovery
│   ├── fomalhaut-user/       # 共享 Linux 用户资料与头像发现
│   ├── fomalhaut-config/     # 共享严格配置
│   ├── fomalhaut-logind/     # 共享非交互 logind 电源 backend
│   ├── fomalhaut-web/        # 协议、主题与 controller
│   ├── fomalhaut-gtk/        # 共享 GTK4/WebKitGTK host 能力
│   ├── fomalhaut/            # greeter 可执行程序
│   └── fomalhaut-lock/       # locker 可执行程序
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

- 定义不依赖 greetd、PAM 或图形工具包的认证事件、prompt、状态和错误。
- 定义权能分离的 `LoginBackend` 与 `ReauthBackend` trait。
- 管理认证回答等敏感数据的生命周期。
- 拒绝非法、重复或过期操作。
- 保留可信 `SessionCommand` 值类型，但不负责发现 session 或启动命令。

core 不负责：

- greetd IPC、PAM handle 或 PAM conversation。
- 用户和 session 列表的视觉呈现。
- WebView 或 JavaScript bridge。
- 从不可信前端解析命令行。
- 保存主题偏好。
- 执行任意电源命令。

### 4.2 `fomalhaut-greetd`

- 承接当前 `fomalhaut-core` 中的 greetd client、wire transport 和 greetd-specific
  状态转换。
- 实现 `LoginBackend`，允许从前端选定用户名，并在认证后启动可信
  `SessionCommand`。
- 隔离 `GREETD_SOCK`、greetd IPC 重试/取消语义与通用认证核心。

### 4.3 `fomalhaut-pam`

- 实现只能重新认证当前 session 用户的 `ReauthBackend`。
- 用户身份由 locker 宿主使用真实 UID/session 推导；API 不接受 username，
  不能启动 session。
- 每次认证在新建的一次性 PAM 子进程 worker 中完成，而不是只放入普通线程；worker
  向 controller 仅传递有界、类型化的 prompt、message 和最终结果，transaction 结束后
  立即退出。

首阶段选用并精确固定 `pam-client 0.5.0` 作为 PAM application/client wrapper。该版本
已经用于 COSMIC/Pop!_OS 的生产锁屏，并有其他真实项目消费者；这说明它具有实际部署基础，
但不等同于上游活跃维护、广泛安全审计或 wrapper 本身能够构成完整安全边界。专项审计发现
其 nullable `set_item` 路径、conversation handler panic、消息数量和 secret 副本清理等方面
不能满足主进程内直接使用的要求，因此首阶段只封装以下已审计 API 子集：

- `Context::new`，且 username 只能来自 locker 已按真实 UID/session 确定的账户；
- `authenticate` 和 `acct_mgmt`；
- 仅在 PAM 策略确有需要时调用 `reinitialize_credentials`。

`fomalhaut-pam` 只作为 library wrapper 使用该依赖，必须禁用 `pam-client` 默认启用的
`cli` feature，避免引入其 `rpassword` 终端交互路径；所有 conversation 都由一次性 worker
中的受限 `ConversationHandler` 驱动。

不得调用 `pam-client 0.5.0` 接受 `None` 的 `set_item`/`set_*` 路径，也不得把该依赖的
unsafe 实现引入 workspace 自有生产代码；自有 crate 继续保持 `unsafe_code = "forbid"`。
Rust 侧持有的回答仍须在可控生命周期内清零，但 wrapper/PAM module 内部的 `strdup` 等副本
无法提供可验证的即时清零保证；一次性 worker 用于限制这些副本、第三方 PAM module 和
transaction 状态的生命周期，不能把这一限制描述成已被消除。

worker panic、异常退出、超时、取消、IPC 断开或协议违规一律 fail closed：locker 主进程
继续持有 session lock，丢弃当前 transaction 及尚未消费的回答，且不得向新 transaction
重放回答；只有显式开始新的认证尝试时才创建新 worker。实现进入可发布状态前，必须用可控
PAM service/module fixture 覆盖 echo-on/echo-off、info/error、密码、OTP/MFA、批量消息、
账户过期、`PAM_MAXTRIES`、取消、超时、worker abort 和回答不重放。该选择不引入 setuid、
直接读取 shadow 或伪认证 fallback。

当前 `fomalhaut-pam` 已实现上述一次性 worker、固定 service/API 子集、有界类型化二进制 IPC、
当前 UID/NSS identity、transaction 取消/超时/断开处理和回答不重放；locker 会在请求 compositor
lock 前预热首个 worker。内存 fake worker 已覆盖多轮 secret/visible prompt、info/error、拒绝后
新 transaction、取消、超时、断开、超限和旧回答不重放。该自动化覆盖不替代受控 PAM
service/module fixture，后者以及真实发行版 PAM stack 验证仍是发布前门槛。

### 4.4 `fomalhaut-session`

- 从受信任的配置目录发现 desktop entry。
- 区分 X11 和 Wayland session。
- 解析显示名称、可执行命令和必要的元数据。
- 将文件系统内容转换为不透明的 `SessionId`。
- 根据策略过滤隐藏、无效或被禁止的 session。

前端只能选择 `SessionId`。实际命令始终由 Rust host 根据已发现的 session 生成。

#### 4.4.1 Discovery 与可信命令映射

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
- session desktop entry 省略 `Type` 时兼容接受，显式存在时只接受 `Type=Application`；这是为
  兼容 Plasma 等由发行版提供、语义明确但省略应用菜单字段的登录 session。仍要求非空 `Name`
  和非空且可解析的 `Exec`。`Hidden=true`、
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
- host 把已经解析并归一化的 UI locale 作为显式 locale 优先级传给 discovery，使 desktop
  entry 的 `Name[...]` 与同一页面语言一致；session crate 仍不自行读取环境变量。
- 生成命令时根据 session 类型设置 `XDG_SESSION_TYPE`，并根据文件名和可选
  `DesktopNames` 设置 `XDG_SESSION_DESKTOP`、`DESKTOP_SESSION` 和
  `XDG_CURRENT_DESKTOP`。X11 wrapper 等发行版策略由后续 host 配置层在可信侧组合。
- discovery 返回可用 catalog 和逐项拒绝诊断；单个损坏文件不阻止其他 session 被发现，
  但目录级 I/O 失败不会被静默吞掉。

### 4.5 `fomalhaut-config`

- 严格解析共享的 TOML 配置，拒绝未知字段并完成路径规范化。
- 将全局配置收窄为 `for_greeter()` 与 `for_locker()` 的角色化、已验证配置，
  不让 locker 读取 session 启动等无关权能。
- 实现通用主题与 greeter/locker 角色覆盖的确定性选择。
- 解析全局可选 `[locale].language`。首阶段只接受 BCP 47 形式的 `en` 与 `zh-CN`；字段省略时
  按 `LC_ALL`、`LC_MESSAGES`、`LANG` 的 POSIX 优先级读取进程 locale，忽略空值，去除编码与
  modifier、把 `_` 视为 `-` 后将所有 `zh` 语言变体映射为 `zh-CN`，其余语言以及
  `C`/`POSIX` 使用 `en`。配置覆盖同时作用于 greeter 与 locker；省略配置时两个独立进程各自
  按启动环境解析。检测失败不得阻止安全启动，稳定回退到英语。

locale 解析不得引入 gettext、ICU 或外部命令依赖。配置 crate 对外只暴露有界 `UiLocale`
枚举以及 Desktop Entry 使用的稳定 locale 候选，不把任意环境字符串送入协议或前端。

### 4.6 `fomalhaut-logind`

- 封装 greeter 与 locker 共同使用的非交互 systemd-logind 电源 backend。
- 将管理员 `PowerConfig` 与 logind `CanPowerOff`、`CanReboot`、`CanSuspend` 返回的 `yes`
  求交集，并只执行 `PowerOff(false)`、`Reboot(false)`、`Suspend(false)`。
- 实现 `fomalhaut-web` 定义的 `PowerControl` seam，但不依赖 GTK、greetd、PAM 或
  session-lock；两个产品宿主不得各自复制一份 D-Bus 实现。

### 4.7 `fomalhaut-web`

- 从主题目录加载静态资源。
- 实现自定义资源 scheme，例如 `fomalhaut://theme/`。
- 在 Rust 类型和版本化 JSON 消息之间转换。
- 实现公共 auth controller 以及权能分离的 greeter/locker controller。
- 定义不依赖 GTK、greetd 或 PAM 的角色化状态快照和事件。

该 crate 不包含正式产品主题。仓库中的 minimal theme 仅用于示例、开发和测试。
它不依赖 GTK/WebKitGTK、`gtk4-session-lock`、greetd 或 PAM。

### 4.8 `fomalhaut-gtk`

- 共享 GTK4 application、WebKitGTK 6.0 `WebView`、资源 scheme、bridge 连接、
  安全 policy、页面 epoch 和 renderer 状态观测。
- 仅接收已经由 `fomalhaut-web` 类型化的请求/输出，不持有 greetd 或 PAM 具体
  backend。
- 不依赖 `gtk4-session-lock`；该依赖只进入 `fomalhaut-lock`，避免迫使 greeter
  引入额外的 session-lock 系统库。

共享宿主切片已经迁入 `fomalhaut-gtk`：该 crate 现在拥有 GTK application 生命周期、
hardened WebView、资源 scheme、bridge、页面 epoch、renderer 观测和辅助内存资源服务，并以
`BridgeController`、有界 `ControllerOutput` 和角色回调接收类型化输入。普通 greeter 窗口、session discovery、
greetd worker 和“登录 session 已启动后退出”的策略仍由 `fomalhaut` 组合；共享 crate
不依赖 greetd、PAM、配置解析或 session-lock binding。角色终态通过泛型 terminal action
交回可执行程序处理，为 locker 后续独占 unlock handle 保留边界。

### 4.9 `fomalhaut` 与 `fomalhaut-lock`

`fomalhaut` 暂时保留现有 greeter 二进制名，组合 `LoginBackend`、session discovery、
greeter controller 和普通 GTK 窗口，继续由 greetd/Cage 启动。

`fomalhaut-lock` 组合 `ReauthBackend`、共享 logind 电源 backend、locker controller 和
`ext-session-lock-v1`。它运行在
已登录的普通用户 Wayland session 中，不执行 session discovery、`StartSession` 或任意
用户切换。只有该宿主持有 session-lock handle 并能最终解锁。

当前 locker 已按这一边界组合 `PamReauthBackend` 与 `gtk4-session-lock 0.4.0`：主进程持有唯一
session-lock instance，每个 monitor 使用独立未 realize 窗口和 WebView，全部页面共享一个
controller/PAM transaction；跨页面只广播序列化事件，关联响应只返回发起页面，一次性
`UnlockAuthorization` 在可信 controller worker 中被消费后才通过 native-only 通道请求解锁。
renderer 或单个页面故障会切换到不依赖 WebKit 的 GTK fallback，并允许为该输出注册新
`ViewId` 重建页面。自动化测试已覆盖多视图 watermark/事件路由、慢页面隔离与单次 native
unlock；真实 compositor 的多输出、hotplug、scale 与异常释放仍须按 TODO 做系统验证。

普通 PAM 拒绝（包括密码错误、账户策略拒绝和达到尝试次数）是可恢复的认证结果：controller
必须保留 WebView，向主题发布 `auth.failed`、公开消息和可重试状态，不得切换可信 GTK
fallback。只有 PAM worker 无法启动、IPC/协议损坏、终态进程无法在有界时间内干净退出、
controller 不变量破坏或 renderer/theme 失效等基础设施故障才能进入可信 fallback。PAM worker
向父进程发送 `Authenticated` 或 `Rejected` 后仍须完成 PAM context 清理；父进程以明确、足够的
有界退出等待确认该终态，不得用过短的清理窗口把正常 PAM 结果稳定误判为 worker 崩溃。内部
诊断只能记录超时、非零退出、IPC、协议等脱敏类别，不记录认证回答或原始 PAM description。

locker 的 GTK application 在 PAM worker 预热、请求 session lock 和收到首个 `monitor` 之间
允许暂时没有任何窗口。这是正常的异步启动阶段，不能依赖 GTK 的窗口引用隐式维持主循环。
`activate` 必须在启动异步工作前取得 `ApplicationHoldGuard`，由 `LockHost` 持有到明确的失败
退出或 compositor 确认授权释放；否则 `activate` 返回后的“零窗口、零 hold”会让 application
以成功状态立即退出，controller 轮询和 session-lock 请求都不会发生。

session-lock surface 使用不注册到 `GtkApplication` 的普通 `GtkWindow`，而不是
`GtkApplicationWindow`；GTK application 只通过上述 hold 管理进程主循环。该边界规避
`gtk4-layer-shell 1.3.0` 与 GTK 4.22+ 的已知失败/解锁路径缺陷：1.3.0 会先 unrealize
application-owned window，再由 `gtk_window_destroy()` 从 application 移除窗口，而 GTK 4.22+
在移除期间仍访问已经失效的 `GdkSurface`，导致 SIGSEGV（上游 issue #122，未发布修复提交
`4419f1b`）。native `destroy` signal 只安排空闲回调清理 Rust `MonitorSurface`，不得在 signal
栈内同步释放最后一个宿主引用。即使未来系统库包含上游修复，也保留这一单一所有权模型，避免
`GtkApplication` 与 session-lock 库同时管理 lock surface 的映射和销毁。

### 4.10 Host controller 与线程边界

真实认证接入采用两层实现，保持 controller 可在无图形环境测试：

- `fomalhaut-web::controller` 通过 backend trait 持有公开状态快照、当前 core
  `PromptId` 和事件 sequence。它接收已经严格解码的 `RequestEnvelope`，输出一个关联
  `ResponseEnvelope` 和按 sequence 排序的 `EventEnvelope` 列表，不依赖 GTK/WebKit。
- 两个可执行程序都在专用 OS 线程中运行 backend/runtime。GTK/WebKit 对象及
  `ScriptMessageReply` 始终留在 GTK 主线程；两侧只通过容量固定的同步通道
  交换可发送的类型。

通道与页面生命周期遵循以下规则：

- GTK 主线程使用非阻塞发送，队列已满时立即向当前请求返回脱敏的 `internal` 错误，不阻塞
  UI，也不创建无界任务。首阶段同时只允许一个未完成 bridge 请求；并发请求被拒绝。
- 每次 `LoadEvent::Started` 都递增页面 epoch、拒绝旧页面尚未完成的 reply，并按通道顺序请求
  controller 取消活动认证。controller 输出携带发起请求时的 epoch；GTK 丢弃与当前页面不
  匹配的输出，防止刷新后的旧响应或事件进入新文档。
- controller 对一个请求的处理是串行事务：调用一次 core 操作，排空该操作产生的 core
  event，先生成必要的 `state.changed`，再生成 prompt/message/succeeded/failed/cancelled
  事件，最后把响应和事件作为一个输出批次交回 GTK。
- 每个 WebView 都拥有 host 内部 `ViewId` 和 `PageEpoch`，页面初始化从带 sequence
  watermark 的快照开始；这两个标识不向 JavaScript 作为权能暴露。
- greeter 的正常窗口退出、renderer 终止和 host 关闭都会发送 shutdown。worker 在退出前检查
  `needs_cancel()`，需要时显式等待 `cancel()`；线程 join 完成后宿主才结束。异常 abort 仍只
  能依靠连接关闭兜底。
- locker 一旦取得 lock 就不因 renderer、主题、PAM worker 或 controller 失败而退出；
  它切换到内嵌的可信 GTK 故障/重试界面并继续持有 lock。

bridge 在 GTK 主线程完成消息总长度检查和严格协议解码，再把 typed request 移入有界队列；
不得把包含认证回答的原始 JSON 复制到跨线程队列。`auth.respond` 使用页面提供的数值只与
controller 保存的当前 core `PromptId` 比较，实际调用 core 时传回原 core ID，不允许前端
构造 core ID。

可信 session 接入继续保持相同的 crate 边界：

- `fomalhaut` 在启动 worker 前运行 `fomalhaut-session` discovery，并把 catalog 中每个条目
  转换为一组前端安全的 `SessionSummary` 和 Rust 内部 `SessionCommand`。主题只能看到摘要；
  命令、参数、环境变量和 desktop entry 路径不进入 JSON 或 GTK/WebKit 对象。
- host 使用经过严格配置语义验证的 session 目录；`sessions` 缺失时使用固定且
  不受进程环境影响的默认目录：按优先级读取
  `/usr/local/share/wayland-sessions`、`/usr/share/wayland-sessions`、
  `/usr/local/share/xsessions` 和 `/usr/share/xsessions`；相对 `TryExec` 只在
  `/usr/local/bin` 与 `/usr/bin` 中解析。不存在的目录继续忽略，目录级 I/O 错误、协议上限
  无法容纳 catalog 或没有任何可用 session 都是启动失败。无论默认值还是显式配置，
  都不通过 `XDG_DATA_DIRS` 隐式改变可信搜索范围。
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

### 4.11 Monorepo 版本与发布

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
- npm CLI 发布 prerelease 时必须显式传入 dist-tag。当前 Semifold package channel 为
  `alpha`，因此 Node.js resolver 的 `npm publish` 固定使用 `--tag alpha`，避免 prerelease
  被拒绝或进入 `latest`。以后通过 `smif config channel` 切换 beta、rc 或 stable 通道时，
  必须在同一变更中同步审核 npm publish 的 `--tag`；stable 发布应恢复适合正式版本的 tag
  策略，不得把旧 `alpha` tag 隐式沿用到新通道。
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
- `semifold ci` step 必须使用稳定 ID `semifold`，所在 job 必须把
  `steps.semifold.outputs['semifold-version']` 与 `semifold-publish` 映射为 job outputs。
  两者是 schema v1 JSON 且一次执行只存在实际分支对应的一个：version output 只描述待应用的
  版本事务，publish output 才描述 package 的实际 `succeeded`、`skipped`、`failed` 或
  `not-started` 结果。下游发布自动化必须消费该结构化事实，不能重新扫描 tag 或从人类可读日志
  推断发布结果。
- Semifold 承诺 `0.3.x` 向前兼容，因此 `semifold-status.yaml` 与 `semifold-ci.yaml` 通过
  `setup-semifold` 的默认行为跟随 latest release，不提供 `version` 输入或保留旧 RC pin。
  两处 workflow 必须保持相同策略；Semifold release 更新后应以本地最新 CLI 验证配置与
  release plan，无需为兼容的 `0.3.x` patch release 逐次修改 workflow。
- Rust registry HTTP pre-check 不显式覆盖 `User-Agent`；正式版 Semifold 由运行时注入包含实际
  engine 版本与项目地址的默认值，避免仓库配置保留与当前 CLI 不一致的旧 RC 版本标识。

本地允许的 Semifold 操作限于 changeset 创建、只读状态查询和配置维护，例如
`smif commit`、`smif status`、`smif config sync` 和 `smif config channel`。本地验证不得
以 dry-run 为理由调用 `smif version` 或 `smif publish`。

初始化迁移时，经用户明确授权，可以把 Cargo 自动生成的共享版本继承手工转换为独立的
`version = "0.1.0-alpha"`。初始化完成后，正常版本变更必须交给 Semifold，不再手工修改
版本号。

### 4.12 Arch Linux 与 AUR 发布

Arch Linux 按两个一等应用和一个独立主题发布三个版本化 AUR 源码包，不使用 `-git` 或
`-bin` 后缀：

- `greetd-fomalhaut` 的主 package 是 `fomalhaut`，只交付 greeter 及 greetd/Cage 示例；
- `fomalhaut-lock` 的主 package 是同名 Rust package，只交付 locker、PAM service、systemd
  user unit 和 compositor/idle 集成示例；
- `fomalhaut-theme-nocturne` 的主 package 是私有 Node package
  `@fomalhaut/theme-nocturne`，只交付构建后的可信静态主题。

三个包不得合并为同一个 AUR package 或 split package：它们的必需依赖、安装场景和上游版本
独立，split package 共享单一 `pkgver` 也无法准确表达 Semifold 的独立版本。需要两种应用角色
和参考主题的用户可以同时安装三包；共享配置路径不由任何 AUR 包静默创建或覆盖。

AUR 自动发布直接消费同一次 `semifold ci` 的 `semifold-publish` schema v1 job output，而不扫描
tag、解析日志或再次请求 registry API。只有 apply 模式的 publish output 才能触发；version
分支 output 不代表 package 已发布。对应用 AUR 包分别维护主 package 与会进入对应二进制的
Rust 依赖集合；主题 AUR 包只跟随其唯一的私有 Node 主 package：

`fomalhaut-logind` 是两个主 package 的共同 Rust 依赖；其 Semifold publish 成功而任一主 package
未发布时，对应的两个 AUR 包都按各自当前主版本增加 `pkgrel`。AUR resolver 的显式依赖集合必须
同步包含该 package，不能只依赖 Cargo 构建时的传递发现。

- publish output 显示主 package `succeeded` 时，使用其 SemVer 同步 `pkgver` 并重置
  `pkgrel=1`；恢复执行中 `skipped` 且 `skip-reason=registry-version-exists` 也可作为该版本已在
  registry 的证据，但如果 AUR 已有相同或更高 Arch 版本则 no-op；
- 主 package 未发布、但至少一个对应依赖 package 已成功发布时，保持 AUR 当前 `pkgver`，以
  同一 Semifold publish commit 的不可变 GitHub source archive 重建，并把当前整数 `pkgrel`
  加一；AUR 尚不存在、版本与当前主 manifest 不一致或状态无法判定时 fail closed；
- `@fomalhaut/theme-nocturne` 是 private Node package，不会发布到 npm；仅对这个精确 package ID，
  `skipped` 且 `skip-reason=private` 表示 Semifold 已确认其版本事务，可以同步主题 AUR 的
  `pkgver` 并重置 `pkgrel=1`。resolver 还必须从同一不可变提交解析
  `themes/nocturne/package.json`，要求名称匹配、`private=true` 且版本与 publish output 一致；
- `failed`、`not-started`、missing-changelog skip、其他 package 的 private skip 和未知
  schema/status 不触发 AUR；
- 仅修复打包时仍允许手动选择目标 AUR 包、不可变 source ref 和更高 `pkgrel`，但不修改 Cargo
  package version，也不在本地执行 Semifold version/publish。

自动与手动 AUR workflow 必须共享一个不取消运行中任务的 concurrency group，使三个仓库的
版本读取、构建审批和推送事务不会并发争用。每次真正开始的 run 都重新读取 AUR 当前版本后再
决定 `pkgrel`；不能按 source SHA、触发方式或目标包拆分 concurrency group，否则两个 run 可能
从相同旧版本计算出相同的下一 `pkgrel`。

`greetd-fomalhaut` 以 greetd、Cage 和 Fomalhaut 组成完整图形登录链路。greetd 不提供 Wayland
compositor，而当前受支持且已经端到端验证的启动命令固定使用 Cage，因此 `greetd` 与 `cage`
都是必需运行时依赖。标准命令直接调用 `dbus-run-session`，所以 `dbus` 同样是必需依赖。包还
依赖 GTK4/WebKitGTK 6.0 及直接链接的 GLib、glibc、libgcc 和 libsoup3；AccountsService 只提供
显示名和头像增强，声明为 `optdepends`。包安装 `/usr/bin/fomalhaut`、许可证、配置文档和
greetd/Cage 示例，但不覆盖 `/etc/greetd/config.toml` 或 `/etc/fomalhaut/config.toml`。AUR 示例
使用 `dbus-run-session`、`cage` 和 `fomalhaut` 的命令名，由 greetd 的受控 PATH 解析，不固化
`/usr/bin`。pacman 安装与升级提示必须要求管理员先审阅并手工应用
`/usr/share/doc/greetd-fomalhaut/greetd-config.toml`，确认配置后停用已有 display manager 并启用
`greetd.service`；包本身不得自动改写配置、切换 display manager、启停服务或结束当前图形会话。

`fomalhaut-lock` 不依赖 greetd 或 Cage；它依赖 PAM、GTK4/WebKitGTK 6.0、提供 session-lock
binding 的 `gtk4-layer-shell`，以及直接链接的 GLib、glibc、libgcc 和 libsoup3。构建
`pam-client` 还需要 Arch `clang`/libclang 工具链。包安装 `/usr/bin/fomalhaut-lock`、
`/etc/pam.d/fomalhaut-lock`、systemd user unit、许可证、配置文档、niri KDL 与通用 swayidle
示例；PAM service 进入 pacman `backup`，升级必须保留管理员修改。

`fomalhaut-theme-nocturne` 是 `arch=('any')` 的纯静态资产包，安装到
`/usr/share/fomalhaut/themes/nocturne`，运行时不依赖 Node.js、Bun、greetd 或 locker；后两者只
作为 `optdepends` 提示可消费该主题。包不创建或修改 `/etc/fomalhaut/config.toml`，管理员通过
`[themes].default = "nocturne"` 按稳定主题 ID 发现，或使用角色覆盖/绝对路径显式启用。项目开发工具链
跟随不承诺向下兼容的 Bun canary，而 Arch 官方 `bun` 是较旧稳定版，因此 AUR 不得用发行版 Bun
冒充受支持构建环境。主题 AUR 改用 Arch `npm`（及其 Node.js 依赖）作为 `makedepends`，通过只在
`packaging/aur/fomalhaut-theme-nocturne` 维护的最小 npm build manifest 与锁文件运行 `npm ci`；
该 manifest 精确镜像 SDK/主题所需外部依赖，但不加入根 Bun workspace，也不替代根
`bun.lock`。npm 安装后先构建本地 SDK，再运行主题的检查、测试和生产构建；主题构建审计脚本
必须只使用 Node/Bun 都支持的标准 API，不能依赖 Bun global。npm/Node 不进入安装后的运行时依赖。

两个应用包的直接 ABI 依赖应根据干净 Arch 构建、ELF `NEEDED` 和 `namcap` 结果滚动维护，不得依赖
偶然的传递依赖。

许可证边界分为两层：Fomalhaut 源码和安装后的软件继续使用 `AGPL-3.0-only`，AUR
`PKGBUILD` 的 `license` 字段也必须声明 `AGPL-3.0-only`；独立 AUR Git 仓库中的
`PKGBUILD`、`.SRCINFO` 和随包提供的打包元数据使用 Arch 推荐的 `0BSD`，以保留未来进入
官方仓库的资格。0BSD 只授权打包脚本，不重新许可 Fomalhaut 源码或二进制。上游仓库中的
AUR 模板和随附的 0BSD 文件必须清楚标明这一作用范围。

AUR 发布由可被 `Semifold CI` 调用、也可手动调度的 reusable GitHub Actions workflow 承担：

- `semifold-ci.yaml` 中执行 `semifold ci` 的 job 暴露 publish output；同工作流下游 job 把该
  JSON 与 `github.sha` 传给本地 reusable AUR workflow。`workflow_run` 事件不携带上游 job
  outputs，不得继续用它跨 workflow 猜测结果。
- reusable workflow 严格验证 schema、dry-run、package/status/version、当前 manifest 与 AUR
  RPC 结果，形成至多三个明确 package matrix entry。外部 HTTP 请求使用项目 User-Agent；
  Semifold publish output 已是 registry 结果权威，不再用易受限流/403 影响的 crates.io API
  curl 作为二次发布探针。
- 发布前使用调用 commit 上的最新打包模板，在干净 Arch Linux 环境分别渲染具体 `PKGBUILD`，
  生成 `.SRCINFO`，应用包使用 Cargo lockfile、主题包使用 `bun.lock` 执行 frozen 构建和目标
  package 测试，并用 `namcap` 检查
  recipe 与产物。实际源码来自已经解析为 commit SHA 的不可变 archive，必须计算 SHA-256，
  不允许 `SKIP`；模板修复与被打包源码因此仍是两个显式输入。
- 验证产物通过 artifact 传递给发布 job。发布 job 必须绑定受保护的
  `aur-production` GitHub Environment，在人工批准后才使用专用 AUR SSH key 克隆并推送
  对应的 `greetd-fomalhaut.git`、`fomalhaut-lock.git` 或
  `fomalhaut-theme-nocturne.git`；AUR 仓库不作为主仓库 subtree 管理。
- AUR maintainer 名称和邮箱使用 GitHub Environment/Repository variables 提供，专用 SSH
  私钥使用 Environment secret 提供。`aur.archlinux.org` 的官方 Ed25519 主机密钥指纹固定在
  受代码审查的 workflow 中；运行时可以用 `ssh-keyscan` 自动取得完整公钥，但必须先计算
  SHA-256 指纹并与固定值进行唯一匹配，匹配成功后才能把扫描结果用作 `known_hosts`。扫描
  失败、出现多个不同指纹或指纹不匹配都必须 fail closed，不能禁用严格主机密钥检查。AUR
  轮换主机密钥时，应先根据官方页面或公告核验新指纹，再通过普通代码评审更新 workflow，
  不需要额外维护 known-hosts secret。workflow 不在日志中输出私钥，不代表用户在本地创建
  AUR package，也不绕过 AUR 的 maintainer 审核责任。

### 4.13 源码工作区安装器

仓库根目录提供可执行的 `install.sh`，用于开发机从当前 checkout 构建并安装 Fomalhaut 与
React 参考主题。它不是 AUR/package manager 的替代品，也不参与发布版本计算；重复运行必须
同时支持首次安装和原地更新。

安装器遵守以下安全边界：

- 真实系统模式只支持 Arch Linux。构建前通过 `pacman -T` 检查固定的构建与运行包集合，只安装
  缺失项；包管理器严格按 `paru`、`yay`、`sudo pacman` 的顺序选择，安装命令使用 `--needed`
  且保留交互确认，不隐式执行全系统升级。`--system-root` 是隔离安装验证，不得修改宿主包状态。
- 自动依赖管理只负责 Arch 系统包，不安装、升级或检查 Rust/Bun 的发行通道；Cargo 与 Bun
  由用户预先提供，安装器仅在构建前确认对应命令可执行。
- 必需包集合覆盖 `base-devel`、构建辅助工具、greetd/Cage/DBus，以及当前直接链接所需的
  GTK4、WebKitGTK 6.0、GLib、glibc、libgcc 和 libsoup3；AccountsService 仍是用户信息增强的
  可选依赖，不由源码安装器强制安装。依赖安装结束后必须重新验证系统包、构建命令、运行时
  绝对路径和 greeter 账户，验证失败时不得开始构建或写系统文件。
  当前 locker 同时要求目标发行版提供 PAM 和 `gtk4-layer-shell` 1.2+ 运行/开发依赖，源码
  安装器在 Arch 上检查并安装 `pam` 与 `gtk4-layer-shell`，不编译隐式私有副本。
- Cargo、Bun 安装与前端构建始终以调用者的普通用户身份执行；脚本拒绝直接由 root 启动，
  只在写系统目录、原子切换文件和可选重启 greetd 时调用 `sudo`。
- 写真实系统前必须确认固定 greetd 命令引用的 `/usr/bin/dbus-run-session`、`/usr/bin/cage`
  可执行，且配置的 greeter 账户可由系统账户数据库解析；验证失败不得生成不可启动的配置。
- 当前 Rust 使用 `cargo build --release --locked -p fomalhaut -p fomalhaut-lock`，安装交易
  同时构建并安装两个二进制。前端先执行
  `bun install --frozen-lockfile` 再调用 workspace 的 `build:theme:nocturne`，不得隐式更新 lockfile。
- `fomalhaut-lock` 的 `pam-client` 依赖通过 `pam-sys` 在构建时运行 bindgen，因此构建环境必须
  提供可加载的 libclang。Arch 源码安装器与 locker AUR 构建安装 `clang`，Ubuntu CI 安装
  `libclang-dev`；这是构建期依赖，不应扩大已安装 locker 的运行时依赖集合。
- 安装必须内容级幂等：构建后的二进制、主题树或 updater 生成的 TOML 与当前安装内容完全相同
  时，分别跳过备份、替换、release 创建和 symlink 切换。确有变化的二进制先写入同目录临时
  文件，保留现有文件的带时间戳备份后通过 rename 切换。变化的主题安装到只读 release 目录，
  默认 prefix 下的 `/usr/local/share/fomalhaut/themes/nocturne` 使用相对 symlink 原子指向同一
  `themes` 目录中的 `.nocturne-releases`；既有普通目录首次迁移时保留为 `legacy` 备份，不递归
  删除旧主题或 release。默认源码安装配置写稳定 ID `nocturne`；非默认 prefix 不属于固定发现根，
  因而写入该 prefix 下主题目录的绝对路径。旧版安装器遗留的 `/etc/fomalhaut/themes/nocturne`
  不再是发现根，也不得继续承载新构建资产。
- `/etc/fomalhaut/config.toml` 与 `/etc/greetd/config.toml` 不允许用 `sed`/正则盲目覆盖整份
  文件。内置 updater 必须先用 Python 标准库 `tomllib` 验证旧内容，只修改脚本拥有的 table/key，
  再验证新 TOML 和预期值；现有文件先生成同目录时间戳备份，临时文件继承 mode/owner，并用
  同文件系统 `os.replace` 与 fsync 原子提交。为避免原子替换悄然改变链接语义，配置文件为
  symlink 或其他非普通文件时必须拒绝修改。两个配置目标的类型和现有 TOML 必须在切换二进制、
  主题或任一配置前完成 preflight；无法解析、重复目标 key 或验证失败必须 fail closed，不能留下
  已知可提前避免的部分安装。只有新旧 TOML 文本确实不同时才创建备份并原子替换。
- `[frontend].path` 兼容期已经结束。源码安装器不再代管理员迁移或删除该 table；配置 preflight
  发现 `[frontend]` 时必须在切换二进制、主题或其他已安装文件前明确失败，并提示管理员将
  `path` 手工迁移到 `[themes].default`。这样安装器与运行时保持同一严格 schema，避免升级过程
  静默解释已经移除的字段。新结构下仍在明确传入缩放参数或首次创建文件时维护
  `[display].scale`。安装器接受互斥的
  两种缩放参数形式：`--display-scale SCALE` 写入 greeter/locker 共用的标量；
  `--greeter-scale SCALE --locker-scale SCALE` 必须成对出现，并写入角色专用的
  `scale.greeter`/`scale.locker` dotted keys。共享参数不得与任一角色参数混用，所有值都在写入前
  使用与 Rust 配置相同的有限浮点数和 `0.5..=4.0` 边界校验。没有显式缩放参数时，升级必须保留
  管理员现有的标量或角色表，不得在两种表示之间隐式迁移。首次创建配置且未传缩放参数时写入
  共享 `scale = 1.0`。安装器另接受 `--language LANGUAGE`，只允许运行时配置支持的精确值 `en`
  与 `zh-CN`；显式传入时通过同一结构化 updater 写入或更新 `[locale].language`，省略时无论首次
  安装还是原地更新都不得创建、删除或改写 `[locale]`，继续由进程环境自动检测或保留管理员已有
  覆盖。首次创建 `/etc/fomalhaut/config.toml` 时还必须写入
  `[power].actions = ["poweroff", "reboot", "suspend"]`，使标准源码安装立即提供经过 logind
  能力过滤的电源菜单；已有配置无论是缺少 `[power]`、显式空数组还是自定义 allowlist，都视为
  管理员策略并原样保留，重复安装和升级不得借机扩大权限。其他 section 和注释同样尽量原样
  保留。greetd 配置只维护 `[default_session].command` 与 `user`，命令使用绝对二进制路径、
  Cage 和独立 `XCURSOR_SIZE`，不再注入 `GDK_SCALE`。
- locker 安装交易还必须安装受管理员控制的 `/etc/pam.d/fomalhaut-lock`
  策略文件和集成示例。升级不得静默覆盖已有 PAM 策略；打包和源码安装应将
  它视为需要审阅与备份的安全配置，而不是主题资源。
- 默认不重启 display manager，避免意外终止当前图形会话；只有显式 `--restart` 才调用
  `systemctl restart greetd`。`--system-root` 允许在临时根目录验证完整安装和配置更新而不写
  主机 `/etc` 或 `/usr`。
- 安装输出按步骤、成功、未变化、备份/迁移和错误使用一致的终端样式；ANSI 颜色只在对应输出
  连接 TTY 时启用，并遵守 `NO_COLOR`。重定向、管道和无颜色环境必须输出不含转义序列的纯文本，
  不得为了样式引入新的运行时工具依赖。

仓库根目录同时提供 `uninstall.sh`，用于卸载曾由默认源码安装器部署的完整 greeter 与 locker
套件；它也在检测到对应 AUR package 时承担从源码安装迁移到包管理安装的职责，但不负责卸载
AUR package 本身。真实 Arch 系统通过 pacman 分别检测 `greetd-fomalhaut` 和
`fomalhaut-lock`，不要求任一 AUR package 预先存在，因此纯源码卸载和只接管一个角色都必须可用；
如果 pacman 声明某个 AUR package 已安装，对应 `/usr/bin` 二进制以及 locker unit 又缺失，则必须
在删除源码文件前失败，避免把仍可工作的源码安装切换到损坏的 package。`--system-root` 隔离验证
以目标 root 中对应的 `/usr` 文件分别模拟 package 接管状态。脚本支持与安装器一致的绝对
`--prefix` 和隔离 `--system-root`，默认 prefix 为 `/usr/local`，真实系统写入仍只通过 `sudo`
完成，并且不得自动重启、启用或卸载任何 system package。

卸载默认删除 prefix 下由源码安装器部署的两个二进制、locker systemd user unit、
idle/compositor 示例、Nocturne 主题 symlink/release 树及这些文件的安装器备份，但保留
`/etc/fomalhaut/config.toml`、`/etc/greetd/config.toml` 和相关配置备份。旧版源码安装器在
`/etc/fomalhaut/themes` 创建的、仍符合受控相对 symlink/release 布局的 Nocturne 当前链接和
release 树也作为源码资产删除；无法证明属于该布局的普通目录和 `legacy` 备份只在后述明确
配置清理确认后删除。保留配置时，只有
`greetd-fomalhaut` 已接管且 greetd 的 `[default_session].command` 仍精确引用旧 prefix 下的
greeter，才在完成 TOML 解析、目标类型检查和结果复验后把它原子迁移到
`/usr/bin/fomalhaut`，同时保留带时间戳备份；需要迁移但无法安全识别或更新时不得删除旧二进制。
检测到 `fomalhaut-theme-nocturne` 接管时，同一结构化更新流程把精确引用旧版 `/etc` 或当前
prefix Nocturne 路径的 `[themes]` 选择迁移为稳定 ID `nocturne`；没有主题 package 接管时保留
管理员配置并明确警告其中可能引用即将移除的源码主题。没有 AUR greeter 的普通卸载保留原配置
文本，并明确警告其中可能仍有失效的 prefix 引用。无论
locker 是否已由 AUR 接管，移除旧的 prefix unit 后都应尽力执行 user systemd daemon reload，
但 reload 失败只给出明确警告，不得回滚已经完成的文件变更，也不得触发锁屏或结束用户会话。

删除系统配置属于独立的破坏性选择：脚本每次运行都必须在任何配置删除前向交互用户列出准确
范围并以默认否定的 `[y/N]` 确认；标准输入不是终端或读取失败时一律按保留处理，不提供绕过确认
的强制参数。只有明确确认后才能删除两个 TOML、它们由安装器生成的备份及旧版 `/etc` 下无法
自动证明为当前源码 release 的 Nocturne legacy 树；prefix 下的可证明源码主题已属于默认卸载
范围，不因保留配置而继续残留。
`/etc/pam.d/fomalhaut-lock` 在 `fomalhaut-lock` AUR package 已接管时无论用户如何回答都不得删除，
pacman 首次接管既有 PAM 策略时产生的 `.pacnew` 也留给管理员审阅；没有 AUR locker 的普通卸载
则把 PAM policy 视为同一确认范围内的安全配置，只在明确确认后删除。脚本不得扫描或修改用户
家目录中的 niri、swayidle 等 compositor 配置；如果未检测到相应 AUR package，只能提示其中的
旧 prefix 引用已经失效，如果已接管则提示迁移到 `/usr/bin`。

### 4.14 `fomalhaut-user` 用户发现与头像资源

用户发现是 Linux 宿主集成，不属于 greetd IPC core。greeter 与 locker 都需要可信用户资料和
同一套头像安全读取规则后，这项能力从 `fomalhaut` 可执行程序的内部模块提取为共享
`fomalhaut-user` crate。该 crate 隔离 AccountsService、NSS 和头像文件系统访问，并把已经
过滤、验证的公开摘要与内存头像资源交给产品宿主；`fomalhaut-core`、认证 backend 和主题均
不得依赖或直接访问 D-Bus、NSS、头像路径或文件系统。

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

locker 不读取 greeter 的 `[users]` 枚举策略，也不枚举或切换账户。它继续先由
`fomalhaut-pam` 根据真实进程 UID 与 NSS 固定认证身份，再由 `fomalhaut-user` 只读调用
AccountsService `FindUserById` 查找同一 UID；返回对象的 `Uid` 与 `UserName` 必须同时匹配
已经固定的身份，才可使用其 `IconFile`。任何 D-Bus、属性、匹配或头像验证失败都只退化为
`avatarUrl = null`，不得改变 PAM 目标、阻止 session lock 或进入认证 fallback。locker 在 PAM
worker 准备完成后即可发起 compositor lock；资料增强和 logind 能力发现位于 GTK 主线程之外，
并可与 compositor lock handshake 重叠，不能把头像 I/O 变成锁屏前的安全延迟。每个 monitor
WebView 收到相同的已验证头像资源和 URI，资源仍由各 WebView 的内存 scheme handler 提供。

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

pub enum AuthEvent {
    Prompt {
        id: PromptId,
        kind: PromptKind,
        message: String,
    },
    Message {
        level: MessageLevel,
        text: String,
    },
    Authenticated(AuthenticatedIdentity),
    AuthenticationFailed,
    Cancelled,
}

pub struct AuthenticatedIdentity {
    // 由可信 backend 构造，不由前端构造。
    // 实际字段不属于前端 wire API。
}

pub struct SessionCommand {
    // 字段不对不可信前端公开。
    command: Vec<String>,
    environment: Vec<String>,
}

pub trait ConversationBackend {
    async fn respond(
        &mut self,
        prompt: PromptId,
        response: Secret,
    ) -> Result<()>;
    async fn cancel(&mut self) -> Result<()>;
    async fn next_event(&mut self) -> Result<AuthEvent>;
}

pub trait LoginBackend: ConversationBackend {
    async fn begin_login(&mut self, username: String) -> Result<()>;
    async fn start_session(
        &mut self,
        command: SessionCommand,
    ) -> Result<()>;
}

pub trait ReauthBackend: ConversationBackend {
    // 认证目标由当前 UID/session 推导，因此没有 username 参数。
    async fn begin_reauth(&mut self) -> Result<()>;
}
```

`PromptId` 由 core 生成，用来拒绝：

- 对已经回答过的 prompt 再次回答。
- 页面刷新后提交的旧回答。
- 在当前状态下无效的并发提交。

`Secret` 应隐藏 `Debug`/`Display` 内容，并在 drop 时尽力清零其 Rust 侧内存。

`LoginBackend` 与 `ReauthBackend` 的分离是权限边界，不只是两个便利 trait。前者能
指定用户名并启动可信 session；后者只能重新认证当前用户，不接受用户名、
不返回 `SessionCommand`、不启动 session。现有 `GreeterClient` 与 greetd transport 在
重构后属于 `fomalhaut-greetd`，不再是 `fomalhaut-core` 的公共边界。

## 6. 认证状态机

目标状态机分为公共 auth state 与角色 lifecycle。公共部分只包含 `Idle`、
`Authenticating`、`WaitingForSecret`、`WaitingForVisible`、`Authenticated`、
`Cancelling` 和 `Failed`，不知道 greetd `StartSession` 或 Wayland lock handle。下图是
当前 greetd login backend 的 lifecycle；重构时其认证部分将映射到公共状态，
`Authenticated → StartingSession → Started` 保留为 greeter 专属阶段。

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
    ├── Error::AuthError ───────► CleanupCancelling
    │                              │ CancelSession::Success/Error（已消费响应）
    │                              ▼
    │                            Failed
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
- 在正常退出、页面失联或 host 主动中止仍然活动的认证路径中，必须显式等待 `cancel()` 完成并
  发送 `CancelSession`。
- greetd 0.10.3 的认证 worker 在 login flow 返回 `AuthError` 时已经终止，但 daemon 的
  `Context.configuring` 槽位仍由后续 `CancelSession` 清除；其参考 greeter `agreety` 也会在
  error 后发送该请求。因此 `AuthError` 必须进入内部 cleanup cancellation，发送且消费一次
  `CancelSession` 响应，再清除当前 prompt、发布可恢复的 `AuthenticationFailed` 并允许新的
  `CreateSession`。由于 daemon 在尝试通知已退出的 worker 前已经从 `Context.configuring` 取走
  旧 session，该清理请求可能返回 `Success`，也可能返回普通 `Error`；两者都表示请求已被 daemon
  处理并应收敛为认证失败，不能把后者升级为认证服务故障。只有 cleanup 请求的 transport/codec
  失败才进入断连状态，且不得自动重放用户名、PAM 回答或后续 `CreateSession`。
- Rust `Drop` 不执行异步 IPC、不阻塞 runtime，也不派生无法等待的后台取消任务；析构只
  清理敏感内存并关闭 transport。连接关闭是异常退出时的最后兜底。
- greetd 连接断开后不盲目重放 PAM 回答。

locker 将认证 lifecycle 与 session-lock lifecycle 分开：

```text
PAM worker ready ─► request lock ─► create one window per output
                                   │
                                   └─ ext-session-lock.locked ─► Locked/ready

Authenticated ─► UnlockAuthorized ─► lock host unlocks
              ─► Wayland/GDK roundtrip ─► Released ─► exit 0
```

只有当 PAM worker 已可用时才能请求 session lock，避免进入锁定后没有认证路径。
宿主只在收到 `ext-session-lock-v1` 的 `locked` 事件后对外报告 ready。每个当前
和新增输出都必须有 lock surface；不得用普通全屏或 layer-shell 窗口代替协议锁。

locker 主进程是唯一能把 `UnlockAuthorized` 转换为 Wayland unlock 请求的组件。
controller、PAM worker 和 JavaScript 都不持有该权能。已锁定后任何 renderer、主题、
controller 或 PAM worker 崩溃都必须 fail closed：宿主保持 lock，显示可信 GTK 故障/
重试界面，必要时重建 worker，但不解锁或自行退出。协议 `finished` 事件必须按
`ext-session-lock-v1` 语义作为 lock 已失效的终态处理；若它在未授权解锁前出现，
必须记录脱敏安全故障并以非零状态结束，不能继续显示伪锁屏。

共具实现必须遵守以下额外约束：

- 同一 backend transaction 同一时刻至多有一个等待回答的 prompt。
- greetd backend 按协议自动确认 `Info`/`Error`；PAM backend 将对应 conversation
  消息转换为不需要回答的公共 `Message`。
- 认证成功、session 启动成功和解锁完成是三个不同概念。
- 对 locker 而言，取消认证从不等于解锁。backend 连接断开或 worker 重建后
  不得盲目重放认证回答。

PAM wrapper 的生态与源码审计已经完成，首阶段采用精确固定的 `pam-client 0.5.0`，并只使用
4.3 节列出的 API 子集。依赖只能在实现 `fomalhaut-pam` 时通过 Cargo CLI 引入；引入依赖
不代表实现门槛完成。一次性子进程隔离、有界 IPC、secret 生命周期限制和完整 PAM fixture
仍是合并 locker 认证实现前必须通过的安全门槛。如果后续版本、API 子集或隔离模型需要改变，
必须先再次更新本文和 `TODO.md`。

## 7. 前端协议

### 7.1 基本原则

- 协议显式携带整数主版本号；首个版本固定为 `1`。
- 请求具有唯一 ID，响应关联该 ID。请求 ID 和事件 sequence 必须是不大于
  `9_007_199_254_740_991` 的非负整数，以便 JavaScript 精确表示。
- 状态事件具有单调递增 sequence，便于丢弃旧事件；sequence 耗尽是 host 的不可恢复错误，
  不允许回绕。
- 只暴露当前 greeter 或 locker UI 所必需的操作，宿主按角色拒绝越权方法。
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

`StateSnapshot` 是以 `mode: "greeter" | "locker"` 为鉴别字段的联合。两种模式
共享 auth state、当前 prompt、有限 message 和事件 sequence watermark；greeter
分支另外包含 users、sessions、已选 session、login lifecycle 与可用能力，locker
分支只包含当前可信 identity、lock lifecycle 与可用能力。locker 快照不得
暴露用户切换、session 列表或 session 启动操作。

公开类型固定为以下形状；字段使用现有 wire 的 `camelCase` 规则：

- `AuthState` 只包含 `idle`、`authenticating`、`waiting_for_secret`、
  `waiting_for_visible`、`authenticated`、`cancelling` 和 `failed`。backend 断开在公开
  协议中收敛为 `failed`，不得重新混入 login/lock lifecycle。
- `LoginState` 包含 `idle`、`starting_session`、`started` 和 `failed`。
- `LockState` 包含 `acquiring`、`locked`、`unlocking`、`released` 和 `failed`。
- `IdentitySummary` 只包含可信 host 提供的 `username`、`displayName` 和可选不透明
  `avatarUrl`，不暴露 UID、home、shell 或 PAM 数据。
- `GreeterStateSnapshot` 固定包含 `mode`、`authentication`、`login`、`prompt`、
  `messages`、`sequence`、`users`、`sessions`、`selectedSessionId` 和 `capabilities`。
- `LockerStateSnapshot` 固定包含 `mode`、`authentication`、`lock`、`prompt`、
  `messages`、`sequence`、`identity` 和 `capabilities`。

`sequence` 是生成快照时已经发布的最后一个 event sequence；尚未发布事件时为 `0`。
页面只接受严格大于该 watermark 的后续事件。`unlocking` 只表示 native host 已接受内部
`UnlockAuthorized` 并开始 Wayland unlock/roundtrip，不把 unlock 权能交给 JavaScript。

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
- `auth.begin`：greeter 仅接受 `{ username }`，locker 仅接受空参数 `{}`。两种宿主
  必须严格拒绝另一角色的参数形式。
- `auth.respond`：接收当前 `promptId` 和 zeroizing 回答。
- `auth.cancel`：无参数。
- `session.select`：只接收不透明 session ID，且只在 greeter 模式可用。
- `power.request`：只接收 `poweroff`、`reboot` 或 `suspend` 枚举。宿主只接受管理员配置
  allowlist 与 systemd-logind 当前无交互授权能力的交集；不在 capability 中的动作返回
  `method_disabled`，执行失败返回脱敏的 `internal` 错误。

请求保持顶层 `{ protocol, id, method, params }` 形式。响应保持顶层
`{ protocol, id, ok, result }` 或 `{ protocol, id, ok, error }` 形式，且只能通过构造器建立
success/error 不变量。无法解析出请求 ID 的畸形 JSON 不生成一个伪造 ID 的响应，由 bridge
记录脱敏诊断并丢弃；已经解析出 ID 的错误必须关联原请求。

公开状态快照的 greeter 分支保留经过过滤的用户摘要、session 摘要和当前选择的
session ID。用户摘要只有用户名、显示名和可选的不透明头像 URL；session 摘要只有
ID、显示名和 X11 / Wayland 类型。capability 由可信宿主按角色生成，主题不得
仅根据 mode 自行假定能力存在。电源功能默认关闭；启用后，greeter 与 locker 都通过共享的
`fomalhaut-logind` backend 在系统 D-Bus 查询 systemd-logind 的 `CanPowerOff`、`CanReboot` 和
`CanSuspend`。只有返回 `yes` 的动作才加入公开列表；`no`、`na`、`challenge`、D-Bus 不可用和
查询失败都按不可用处理。Fomalhaut 不运行 Polkit agent，也不为任一角色发起交互授权。

greeter 与 locker 的 `state.get` 分支都必须携带不可变的 `locale`，其类型为协议生成的
`UiLocale = "en" | "zh-CN"`。locale 由 host 配置层确定，不接受主题回写，也不需要动态事件；
配置或进程环境变化在宿主重启后生效。该字段进入 JSON Schema、ts-rs 生成绑定和 SDK 的
`StateSnapshotFor<M>`，SDK 在 bootstrap 时拒绝缺失或未知 locale，避免 TypeScript 类型与实际
wire 值分离。第三方主题可以按该字段选择自己的资源或消息目录，但不得把浏览器猜测置于宿主
配置覆盖之上。

收到已发布能力对应的请求时，controller 先取消仍在进行的角色认证并清理 prompt：greeter
取消 greetd session，locker 取消当前一次性 PAM transaction。随后通过 systemd-logind 的
`PowerOff(false)`、`Reboot(false)` 或 `Suspend(false)` 执行动作。这里的 `false` 明确禁止
D-Bus 方法发起交互授权。locker 发起电源操作时不产生 unlock authorization，也不释放
session-lock。取消发生在仍然存活的页面上下文时，controller 必须发布 `state.changed = idle`
和 `auth.cancelled`，使主题立即清除旧 prompt；页面不得在后续任何生命周期中重新提交已经取消的
`promptId`。

locker host 还必须订阅 systemd-logind `PrepareForSleep(bool)`：收到 `true` 时取消仍然活动的
PAM transaction 并向所有 monitor 页面广播取消结果；收到 `false` 且 compositor lock 仍然有效
时，只启动一次全新的 PAM transaction，并广播其新 prompt。恢复路径不得重放休眠前的用户名、
回答或 prompt ID，也不得产生 unlock authorization，除非这次新的 PAM transaction 本身完成了
认证。重复、延迟或与 locker 自身 `Suspend(false)` 请求交错的 sleep 信号必须按 controller
串行顺序收敛，不能创建并发 transaction。若 sleep 信号监控不可用，locker 仍保持 fail closed
和 session-lock；主题在公开状态为 `idle` 时必须清除 prompt 并提供显式重新认证入口，不能保留
一个看似可输入但必然 stale 的密码框。内嵌 minimal theme 与参考主题都必须覆盖该降级行为。

因此 suspend/resume 后必须仍由 compositor lock 覆盖，并且恢复认证只使用新 PAM transaction。
电源后端故障不得使 greeter 或 locker 启动失败：启动时退化为空 capability；
请求与能力查询之间发生竞态时，调用失败只返回稳定、脱敏错误，不回退到 `systemctl`、shell
或任意命令执行。

v1 事件至少包含：

- `state.changed`
- `auth.prompt`
- `auth.message`
- `auth.succeeded`
- `auth.failed`
- `auth.cancelled`
- `session.selected`
- `session.started`
- `lock.acquired`
- `lock.failed`
- `lock.released`

`session.*` 只能由 greeter 发出，`lock.*` 只能由 locker 发出。多输出 locker 中的
`ViewId` 与 `PageEpoch` 是 host 内部路由信息，不进入 JavaScript 协议；各页面通过
带 sequence watermark 的 `state.get` 快照与后续事件完成一致初始化。

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

greeter 和 locker 只维护这一个 SDK，不新建 `fomalhaut-lock-sdk`。SDK 导出
`RuntimeMode = "greeter" | "locker"` 和与 Rust wire 一致的
`GreeterStateSnapshot | LockerStateSnapshot` 判别联合。Client 首先从 `state.get`
建立当前 mode，在 TypeScript 收窄后返回角色 facade：greeter 中为
`auth.begin(username)`，locker 中为 `auth.begin()`。运行时仍由 host 严格检查角色，
类型收窄不是安全边界。

SDK 的主收敛机制固定为泛型 `FomalhautClient<M extends RuntimeMode>`。异步 factory 先完成
`state.get` bootstrap，返回
`FomalhautClient<"greeter"> | FomalhautClient<"locker">`；调用方按只读 `mode` 字段收窄。
`StateSnapshotFor<M>`、`AuthBeginArgs<M>`、角色事件名/数据和 session facade 都由条件类型或
`Extract` 从生成 wire 类型推导，不重复手写第二套协议类型。greeter 实例的
`auth.begin` 参数 tuple 为 `[username: string]`，locker 为 `[]`；`session` 在 locker
实例上收敛为 `undefined`，不能调用。factory 设置 snapshot watermark 后才向订阅者发布更新
事件，并拒绝 bootstrap 前或 mode 不匹配的状态结果。

Node/TypeScript 开发工具链统一使用 Bun，不在根目录或产品 workspace 维护 npm、pnpm 或 Yarn
lockfile。根 `package.json`
以 `workspaces = ["packages/*", "themes/*"]` 分别发现通用 package 与主题 package，根 package
必须为 private；`bun install` 产生
并提交文本格式的 `bun.lock`，CI 使用 `bun install --frozen-lockfile`，禁止隐式迁移或在根目录、
`packages`、`themes` 中同时提交其他包管理器 lockfile。

包管理器约束保留两个有边界的发行例外。第一，Semifold CI 可使用其 Node.js resolver 默认生成的
`npm publish --provenance --access public`，以支持 npm trusted publishing/OIDC provenance。
除第二项 AUR 构建例外外，npm 不参与依赖安装、workspace 解析、脚本、测试、构建或 lockfile
生成；本地与 Agent 也不得执行 publish，该命令只能由 GitHub Actions 中的 `semifold ci`
间接调用。根 private package
不登记为 Semifold package；Node.js resolver 同步可发布的 `packages/fomalhaut-sdk` 与各私有
H5 主题，后者由 Semifold 管理 changeset、版本和 changelog，但发布时依据 `private` 标记跳过。
第二，Nocturne AUR 源码包按 4.12 的隔离 build manifest 与 lockfile 使用 `npm ci`，解决 Arch
稳定 Bun 与项目 canary 不兼容的问题。这个例外只存在于 `packaging/aur`，不得生成根
`package-lock.json`、改变开发 workspace 解析或扩大为普通本地构建路径。

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

`fomalhaut-sdk` 在生成 wire types 上提供手写、框架无关的 Client。公共 API 至少覆盖
`state.get`、角色化 `auth.begin`、`auth.respond`、`auth.cancel` 和带判别联合收窄的
事件订阅；`session.select` 只存在于 greeter facade。Client 的 `power.request` 只接受生成类型中的
`PowerAction`；主题必须先读取 `state.get` 的 capability，只展示其中存在的动作。Client 内部管理 request ID、验证
响应关联、协议版本和单调 event sequence，并把协议拒绝、bridge 失败和本地 busy 分成稳定
错误类型。

SDK 通过可注入的 `FomalhautTransport` 隔离宿主，默认 `WebKitTransport` 封装
`window.webkit.messageHandlers.fomalhaut` 与 `fomalhaut:event`。这允许 Node 单元测试、未来
demo transport 或其他宿主复用 Client。Client 同一时刻只允许一个请求，不自动排队认证回答，
避免 secret 因排队在 JavaScript 闭包中延长存活；SDK 不记录请求 body，主题仍须在提交后立即
清空输入元素。首阶段 SDK 保持零运行时依赖、纯 ESM，并由 TypeScript compiler 生成 JavaScript
和 declaration 文件，不引入 bundler。

多输出 locker 会在多个 WebView 中同时初始化同一份 SDK。每个 Client 先获取带
sequence watermark 的快照，再只接受更新的事件；全局 controller 是认证状态的
唯一事实来源，任一页面都不能拥有独立 PAM transaction。`ViewId`/`PageEpoch`
由 host 管理。任何主题都必须在调用 SDK 前同步清空密码 input；密码继续
进入 JavaScript 的已知限制保持不变。

所有手写和生成的 TypeScript 都由仓库锁定版本的 Biome 统一处理。生成命令先运行 `ts-rs`，
再对 generated 目录执行 Biome format，随后执行只读 check；CI 使用 Biome `ci`、TypeScript
typecheck、SDK 单元测试和 build，并在重新生成后以 Git diff 检查产物已提交。生成目录不得整体
关闭 linter；只能为生成器无法规避的问题添加有说明的最小规则 override。Biome 和 TypeScript
随项目滚动升级到最新稳定版，但每个提交必须通过精确依赖版本与 `bun.lock` 保持可复现。所有
脚本通过 `bun run` 调度，SDK 测试使用 `bun test`；首阶段 build 仍由 TypeScript compiler 生成
标准 ESM JavaScript 和 declaration，不为仅有的库代码额外引入 bundler。

## 8. 前端和主题

正式前端由系统管理员通过全局配置提供。Fomalhaut 不要求 React、Vue、Svelte
或任何包管理器；
只要求配置目录最终包含浏览器可加载的静态资源。

示例配置：

```toml
[themes]
default = "nocturne"
greeter = "custom-greeter"
locker = "/srv/fomalhaut/themes/custom-locker"
```

`greeter` 和 `locker` 都是可选覆盖。字段接受稳定主题 ID 或绝对主题目录；不以 `/` 开头的
值必须是合法 ID，相对路径仍然无效。主题选择优先级固定为“角色专用 →
`default` → 内嵌 minimal theme”：因此正常部署只需要一个通用主题，同时也
允许管理员为两个角色分别选择主题。同一目录的 `theme.toml` 仍只有一个
`entrypoint`，页面通过 SDK 的 `mode` 分支呈现，不在主题清单中增加两个入口。

`[frontend].path` 兼容期已经结束；`[frontend]` 现在与其他未知顶层字段一样由严格配置解析直接
拒绝。管理员必须在启动或升级前显式改用 `[themes].default`，运行时和安装器均不得继续提供
别名、弃用警告或自动迁移。

外部主题目录必须包含主题清单：

```toml
[theme]
id = "my-theme"
name = "My Theme"
protocol = 1
entrypoint = "index.html"
```

`theme.id` 是配置和发现使用的稳定机器身份，长度为 1–64 字节，只接受小写 ASCII
kebab-case：一个或多个 `[a-z0-9]+` segment 以单个 `-` 连接。主题发布后不得仅因展示文案、
品牌或本地化调整修改 ID。`theme.name` 继续是最长 256 字节、无控制字符的展示名称，可以独立
调整；显示名称不参与来源认证、冲突消解或配置匹配。缺失或非法 ID 的外部主题清单无效。

按 ID 选择时，宿主通过共享发现模块只枚举固定 root-owned 搜索根的直接子目录，不递归扫描，
也不读取 `$HOME`、XDG 目录、环境变量或网络来源。搜索根和优先级固定为：

1. `/usr/local/share/fomalhaut/themes`，代表管理员的本地/源码安装；
2. `/usr/share/fomalhaut/themes`，代表 AUR、发行版或其他系统 package 安装。

发现器以每个候选受 16 KiB 限制的 `theme.toml` 中 `theme.id` 做精确匹配。同一搜索根存在多个
相同 ID 时按直接子目录路径的字节字典序选择；本地根始终优先于系统根。出现多个匹配时宿主必须
记录脱敏冲突警告和最终绝对路径。候选一旦按上述顺序选中，完整清单、protocol、entrypoint 或
资源验证失败必须使启动 fail closed，不得静默尝试较低优先级副本。搜索根不存在视为空；存在但
无法安全枚举则发现失败。绝对路径选择绕过发现优先级但仍执行完全相同的清单和 capability 校验。

主题加载规则：

- 外部主题是管理员选择的受信任代码，而不是安全沙箱中的不可信内容。主题 JavaScript 能读取
  用户在页面中输入的用户名、PAM 回答和其他认证信息，因此当前版本只适合安装来源可信、内容
  已审查的主题。资源 capability、CSP 和导航限制用于缩小误配置与文件暴露面，不构成对恶意
  主题代码的完整隔离；主题来源验证、签名或打包机制留待后续安全加固。
- `/etc/fomalhaut/config.toml` 不存在或当前角色没有配置任何可用主题时使用内嵌
  minimal theme；文件存在但无法读取、解析或
  通过语义验证时明确失败，不静默回退。配置指定外部主题时，缺失/损坏的 `theme.toml`、
  不支持的 protocol 或无效入口同样是启动失败。运行中某个资源消失只返回脱敏的资源错误。
- 发现后的外部主题根必须是绝对目录。host 使用 `cap-std` 打开一次目录 capability；发现阶段
  同样以搜索根 capability 枚举直接子目录并有界读取清单，主题清单和所有
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

现有内嵌 minimal theme 已在同一个单页中为 greeter 与 locker 提供最小可操作界面，
而没有另建一套主题规范。它仍是示例而不是
固定产品 UI，并遵守：

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
- 根据 `state.get.locale` 在完整的英语与简体中文消息表之间切换，设置文档 `lang` 并使用同一
  locale 格式化日期；首次快照前只允许用 `navigator.languages` 选择 loading/bridge-failure
  文案，收到快照后必须以宿主 locale 为准。认证 backend 提供的 prompt 与 message 是外部认证
  栈文本，默认必须原样安全展示，不做机器翻译；唯一的展示归一化是 `secret` prompt 中严格匹配
  ASCII `Password` 或 `Password for <非空目标>`（忽略大小写、首尾空白和可选末尾冒号）的标准
  密码提示，两个角色都必须改用主题目录中的 `Password`/`密码`，避免泄露目标名称并保证同页一致。
  其他 secret、visible、OTP、PIN、自定义 prompt 和 info/error message 继续原样展示，不能仅因
  `secret` 类型就假定输入一定是密码。
- 使用原生 label、form、input、select、button、`aria-live` 和键盘提交提供最小无障碍能力。
  greetd 返回认证错误后，Core 必须先发送并消费一次 `CancelSession` 响应以清除 daemon 的旧
  configuration 槽位；该 cleanup 返回 `Success` 或普通 `Error` 都收敛为认证失败，随后清除旧
  prompt 并向前端发布失败状态。登录失败或主动取消后恢复用户名输入，session 启动成功由 host
  退出，不由页面导航处理。

该嵌入式主题已在真实 WebKitGTK/Wayland 实例中验证：allowlist 依次加载 HTML、CSS 和外部
JavaScript，脚本初始化后通过正式 bridge 发出 `state.get`；资源不需要网络、内联脚本或
宽松 CSP。认证与 session 行为继续由 controller 和真实 Unix socket stub 的全流程测试覆盖，
真实 PAM 输入则留给 greetd/Cage 系统测试，避免在开发会话中模拟用户密码。

### 8.1 React 参考主题

仓库在 `themes/nocturne` 维护 ID 为 `nocturne`、显示名为 `Fomalhaut Nocturne` 的官方参考主题。主题源码包使用
`@fomalhaut/theme-nocturne` 名称，并始终保持 `private = true`；它属于 Bun workspace 和
Semifold package 列表，以便参与统一的 changeset、版本与 changelog 管理，但 Semifold 发布阶段
必须根据 package 的私有标记跳过 npm 发布。`themes/<id>` 是仓库内多主题源码布局，
`@fomalhaut/theme-<id>` 是同类 H5 主题包的命名约定，面向用户的显示名称则来自各主题
`theme.toml`；稳定 ID 参与配置和固定系统目录发现，显示名称只是主题自声明元数据，不构成来源
或安全认证。该主题用于证明
`fomalhaut-sdk` 能支持完整的框架前端，并向主题作者提供可构建示例；它不
嵌入 Rust 二进制、不替代无构建依赖的内置 minimal theme，也不改变用户通过
`[themes]` 选择通用或角色专用可信静态主题的能力。生产产物是 `dist/` 下的纯静态目录，根目录
包含 `theme.toml` 与 `index.html`，管理员可以直接让配置指向该绝对路径。正式 Arch 用户也可
通过独立的 `fomalhaut-theme-nocturne` AUR 源码包构建并安装相同产物；该分发路径不改变主题
Node package 的 private 状态，也不构成 npm 发布。

参考主题当前已在同一 `index.html` 中使用 SDK `mode` 支持两种角色：greeter 保留用户/session 选择，
locker 只展示当前 identity、公共多轮认证和 lock lifecycle，不显示或调用用户/
session 切换。管理员仍可用 `[themes].greeter`/`locker` 选择两个不同目录。

参考主题固定采用 React、TypeScript、Vite、Tailwind CSS v4、shadcn/ui Luma style 与
Zustand。主题源码 manifest、workspace 依赖和开发脚本继续只由 Bun canary 管理；4.12 的 AUR
npm manifest 是经测试同步的隔离打包镜像，不参与主题 workspace。Vite 使用官方 React 与
Tailwind Vite plugin，
并设置 `base = "./"`，确保所有构建资源相对于 `fomalhaut://theme/` 加载。项目不引入 router、
SSR、服务端数据获取、CSS Modules、Sass、CSS-in-JS、远程字体或网络资源。shadcn 组件使用
CSS variables 和 Luma 的圆角、柔和层级与宽松布局基础。session 选择使用 shadcn/ui Luma
`Select`，不使用浏览器原生 `select`，避免 WebKit 与普通浏览器的 UA 样式差异。该组件允许
使用 Base UI 自身的 portal/positioner；浮层仍只能存在于当前可信主题文档中，不允许新窗口、
导航或放宽宿主 CSP。项目源码继续禁止手写 `style` prop 和内联 `<style>`。

所有项目自有文件与目录使用 ASCII `kebab-case`；`package.json`、`components.json`、
`tsconfig.json`、`index.html` 等生态固定单词文件名继续保持小写，Vite 配置显式命名为
`vite-config.ts`。Semifold 在发布分支生成的标准 `CHANGELOG.md` 是固定文件名例外，文件名
审计必须接受它，同时继续拒绝其他未经列举的 PascalCase/camelCase 路径。TypeScript 类型和
React component 标识符仍使用语言惯例的 PascalCase。项目添加文件名审计测试，阻止后续引入
PascalCase/camelCase 文件名。组件样式只使用 Tailwind
utility 与 shadcn semantic token，不允许 `style` prop、内联 `<style>` 或手写 component
selector；动态或较长的 `className` 必须通过 shadcn 提供的 `cn()` 分组组合。

前端只通过 workspace 中的 `fomalhaut-sdk` 访问宿主。SDK runtime 负责 client 生命周期与全部
v1 事件订阅，Zustand vanilla store 保存公开状态快照、选择、busy 和脱敏错误，并通过 React
provider 注入，便于 mock transport 测试。store 不使用 persist/devtools middleware，不写
localStorage/sessionStorage，不保存或记录 PAM 回答。认证输入使用不受控 DOM input：提交时先
读取值、同步清空 DOM 并释放页面侧引用，再调用 SDK；JavaScript 字符串无法可靠清零的限制
仍然成立。

参考主题与内嵌 minimal theme 一样以 `state.get.locale` 为最终语言来源，并完整提供英语和
简体中文界面文案。React/TypeScript 侧使用成熟的 `i18next` 与 `react-i18next`，把内置 resources
纳入 `CustomTypeOptions` module augmentation，使组件中的消息 key、namespace 和返回值受
TypeScript 检查；不得在组件中散落未受目录管理的界面字符串，也不得因缺少翻译而静默回退。
首次快照前的 loading/fatal 文案可以按 `navigator.languages` 临时选择；收到快照后必须调用
i18next 切换到宿主 locale、同步 `<html lang>`，并让日期/时间格式使用同一 locale。资源全部随
可信主题 bundle 提供，不启用网络 backend、cookie/localStorage cache 或运行期资源下载。
React 页面必须用同一个类型化 helper 为 greeter/locker 的标准密码 prompt 选择 `form.password`；
其他 PAM prompt/message 与宿主返回的诊断文本仍按可信纯文本原样显示，不做机器翻译。无框架、
无构建步骤的内嵌 minimal theme 必须实现相同的窄匹配规则并继续使用自身完整的静态双语目录，
不为此引入运行期依赖。

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

该页面是设备登录/锁屏界面而非可复制阅读的文档，普通标题、标签、状态文本和控件文案默认
不得被指针拖拽选中。Nocturne 必须在全局基础样式中禁用普通页面内容的文本选择，同时为
`input`、`textarea` 和启用的 `contenteditable` 区域显式恢复文本选择，保证编辑、移动光标和
选择输入内容不受影响；这一规则不得改变键盘 focus、表单输入或无障碍语义。自动化验证必须
同时覆盖全局禁用规则及可编辑元素例外，避免后续组件重新暴露整页文本选区或禁用输入选择。

参考主题必须在 greeter 普通窗口与 locker session-lock surface 上保持同一视觉结构，同时把
锁屏输入响应放在装饰效果之前。全屏夜空使用直接绘制的静态多层 CSS gradient，不使用覆盖
viewport 的 `filter: blur()` 元素；输入、按钮、菜单和用户 tile 使用半透明实色与边框，不在
每次键盘、指针或 focus 重绘时叠加 `backdrop-filter`。短时 opacity/transform 过渡与 loading
spinner 可以保留，但不得引入连续背景动画。自动化检查应阻止参考主题重新引入 viewport 级
blur 或 backdrop-filter；最终流畅性仍须在高 DPI/高刷新率的真实 greeter 与 locker 中验证。

普通浏览器中的 Vite 开发服务器没有 WebKit bridge，因此项目提供只在
`import.meta.env.DEV` 分支动态加载的 `development-transport.ts`，以实现
`FomalhautTransport` 并模拟公开状态、prompt、失败、取消和事件。它只是主题开发 fixture，
并公开 `poweroff`、`reboot`、`suspend` 三项模拟 capability，以便在普通浏览器中预览完整电源
交互；模拟 `power.request` 只返回成功，不得访问宿主、systemd-logind 或真实电源接口。它不等同于
宿主级 demo mode。生产构建必须 dead-code eliminate 该 transport；缺少真实 bridge
时显示拒绝式错误，不能静默使用模拟认证。项目自有源码禁止调用 `fetch`、WebSocket 或其他
网络 API；构建测试检查产物没有 demo 标记，检查 HTML/CSS 没有远程 URL、inline
script/style、form navigation 或绝对资源 URL，并确认所有资源小于宿主 8 MiB 上限且清单位于
产物根目录。生产 JavaScript bundle 不采用简单的 `fetch(` 字符串禁令，因为 ReactDOM 19
自身包含 stylesheet preload 的内部 `fetch` 实现；它不是主题发起网络访问的授权边界。网络
隔离仍由主题源码审查、静态资源引用检查以及宿主 CSP/WebKit policy 共同强制执行。

Nocturne 的电源操作列表使用 shadcn/ui `DropdownMenu`，危险操作确认使用独立的
`AlertDialog`；菜单必须支持点击外部或 Escape 关闭、键盘导航和关闭后的焦点恢复，确认层保持
模态语义。主题不得用自维护 document listener 或绝对定位容器重新实现这些基础交互。

测试至少覆盖 store 初始恢复和事件转换、零/多用户选择页、单用户跳过选择页并启动 PAM、居中
用户集合、已知用户与其他用户分支、身份未知的活动认证恢复、session 选择、secret/visible
多轮 prompt、回答在异步请求完成前已从 DOM 清空、busy 背压、取消失败不离开认证页、头像
fallback、文件命名和生产构建契约。CI 通过 Bun 运行 Biome、TypeScript、Vitest 和 Vite build；
生产构建审计脚本本身保持 Node/Bun 双运行时兼容，使 AUR 可以复用同一安全检查而不依赖 Bun；
最终还必须在 WebKitGTK 自定义 scheme 中验证 module script、CSS 与分块资源加载。

## 9. WebView 运行环境

greeter 和 locker 宿主固定使用 GTK4 + WebKitGTK 6.0，并通过 Rust `gtk4` 与
`webkit6` 原生绑定直接调用。当前阶段只实现和维护该统一技术栈，不并行
实现 WPE WebKit。

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
- `fomalhaut-gtk` 共享 GTK application、WebView 生命周期、原生信号与安全 policy；
  `fomalhaut` 和 `fomalhaut-lock` 分别组合普通 greeter 窗口与 session-lock 窗口。
- GTK 和 WebKit 对象只在创建它们的 GTK 主线程访问。WebView 回调不得阻塞等待 greetd；
  backend 集成通过有界消息通道把请求交给 controller，再把序列化后的结果投递回
  GLib 主上下文。

greeter 继续使用 Cage 中的普通 GTK 全屏窗口。locker 使用 `gtk4-session-lock 0.4.0`
封装 `ext-session-lock-v1`；该 binding 的 GTK4 0.11/GLib 0.22 依赖与当前 workspace
`gtk4 0.11.4` 基线一致。其底层 C 库 `gtk4-layer-shell` 从 1.1 起支持 session
lock，1.2 起提供所需 monitor API；因此 locker 应以 1.2+ 为最低能力基线，
当前目标 Arch 环境已通过 pkg-config 验证 1.3.0，并完成 native crate 编译。这个库在此处用于获得 session-lock 实现与
monitor API，不意味着允许用 layer-shell surface 模拟锁屏。

locker 为每个 monitor 创建尚未 realize 的新 `GtkWindow` 和 WebView，然后将其交给
session-lock API；输出移除后销毁对应窗口，不重用已绑定或已销毁窗口。主题
弹层必须使用页面内元素；WebKit/GTK popup 不能作为跨 lock surface 的可靠组件。

应用侧最初使用内置探针页面验证宿主能力；完成真实 core、可信 session、
严格配置和外部主题接入后，该资源已演进为上一节定义的嵌入式 minimal
theme。当前 greeter 已连接真实 greetd 并能读取管理员配置的主题目录，
继续维持以下已经验证的宿主边界：

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
  greeter 可在取消认证后非零退出交给 greetd 恢复；locker 在 `locked` 之后必须改用
  可信 GTK fallback 并保持 session lock。

嵌入式 minimal theme 只为可操作认证和协议示例提供基线。外部主题目录、配置与
清单检查由两个角色复用；角色化主题选择、locker 模式以及不依赖 renderer 的内置可信 GTK
故障页面均已接入。真实 compositor 上的故障恢复仍须单独验证，不能只以 controller 或主题
单元测试替代。

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

## 10. greeter 与 locker 的 Wayland 启动

greeter 的 WebView 需要图形环境。推荐让 greetd 启动一个极简 Wayland compositor，
再由 compositor 启动 Fomalhaut。例如：

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

`fomalhaut-lock` 不由 greetd 或 Cage 启动，而在已登录用户的现有 Wayland session
内运行，使用该 session 的 `WAYLAND_DISPLAY` 与用户 D-Bus/logind 上下文。启动时如果
compositor 未广告 `ext-session-lock-v1`，必须在请求锁之前明确失败，不回退到
普通全屏、layer-shell 或桌面环境私有伪锁屏。

swayidle、systemd user service 和 suspend hook 集成必须能等待 locker 发出可验证的
readiness，且该信号只在 compositor 发出 `locked`、locker controller 已记录
`lock.acquired` 后产生。首阶段 `fomalhaut-lock` 始终作为前台进程运行，不提供自行
fork/background 模式；仓库提供 `Type=notify` 的 systemd user service，locker 使用标准库
Unix datagram 向 systemd 提供的 `NOTIFY_SOCKET` 发送 `READY=1`。服务使用
`TimeoutStartSec=0`，因此 `systemctl --user start fomalhaut-lock.service` 只在真正锁定后返回，
初始化失败则明确失败，锁定后的通知故障也不会因启动超时杀死 locker。该 systemd unit 是
compositor-neutral 的唯一推荐启动入口；niri 快捷键使用 `spawn "systemctl" "--user" "start"
"fomalhaut-lock.service"`。自动 idle/lock/before-sleep 可使用能在 niri 等 Wayland compositor
运行的通用 `swayidle` daemon，并让三个 hook 都调用同一阻塞启动命令；`swayidle` 的名称不代表
locker 或集成只支持 Sway。挂起前必须等待命令成功，避免设备已挂起但 session 尚未锁定的竞态。
直接启动二进制时没有 systemd readiness 消费者，进程仍保持前台直到授权解锁。

locker 的 PAM stack 可能按发行版策略执行已有的特权校验 helper；当前 Arch
`pam_unix.so` 会透明调用 setuid `unix_chkpwd` 读取受保护的密码数据库。因此 user unit 不得设置
`NoNewPrivileges=yes`，该选项会通过 `execve` 阻止 helper 获得其文件授予的身份并使正确密码也
无法验证。真实 systemd user scope 验证还表明，`LockPersonality=yes` 与
`RestrictSUIDSGID=yes` 都会为了安装各自的 seccomp 过滤器而隐式把进程的 `NoNewPrivs` 设为 `1`；
同一 unit 中显式写入 `NoNewPrivileges=no` 也不能覆盖该内核状态。因此当前内嵌一次性 PAM worker
架构下，推荐 unit 显式保留 `NoNewPrivileges=no`，并且不得设置这两个选项或其他会隐式启用
`NoNewPrivs` 的 systemd hardening。Fomalhaut 自身二进制仍无 setuid bit、不直接读取 shadow，也不
实现 PAM 之外的提权或认证 fallback；如果未来要求恢复与 setuid helper 冲突的 seccomp hardening，
必须先把 PAM 调度迁入具有独立认证、IPC 和权限边界的专用 service/broker，不能在现有 unit 中伪称
两者可以兼容。

该 user unit 还必须使用 `UnsetEnvironment=GDK_SCALE GDK_DPI_SCALE` 清除 user manager 可能继承的
工具包缩放变量。locker 的基础输出缩放由现有 compositor/GTK 负责，额外 WebKit zoom 只来自
`[display].scale` 的 locker 值，避免 shell 或桌面环境变量与角色配置重复叠加。该清理只影响
locker 服务进程，不修改用户会话的全局环境。

首阶段 locker 只承诺兼容选定的 `ext-session-lock-v1` compositor。X11 和由桌面环境
内建、不允许第三方 session-lock client 的平台属于显式兼容性边界，不宣称支持。

## 11. 安全模型

### 11.1 信任关系

受信任：

- greetd 及其 Unix socket。
- 系统 PAM stack、为 locker 安装的受控 PAM service 与经审计的 PAM wrapper。
- Wayland compositor 及其 `ext-session-lock-v1` 实现。
- Fomalhaut 安装的 Rust 二进制和系统配置。
- 管理员明确配置的 session 目录及 desktop entry。
- 管理员明确选择并审查的主题 HTML、CSS 和 JavaScript。

默认不信任：

- WebView 导航目标。
- 前端传来的所有字符串、ID 和操作顺序。
- desktop entry 中未经策略验证的字段。

管理员安装自定义主题意味着接受该主题可以读取用户在其页面中输入的内容，但不意味着主题
自动获得系统命令执行或 greetd socket 访问权限。

### 11.2 必须实施的防护

- greeter 只以专用低权限 `greeter` 用户运行；locker 只以当前普通 session
  用户运行。两者都不安装 setuid bit，不自行读取 `/etc/shadow`；locker 允许系统 PAM stack
  按管理员策略执行发行版已有的 `unix_chkpwd` 等 helper，因此不得用 `NoNewPrivileges` 破坏
  PAM 的既有权限模型，也不得在同一 user unit 中启用会隐式设置 `NoNewPrivs` 的 seccomp
  hardening。需要更强进程沙箱时必须先拆分 PAM broker 权限域。
- 正式模式不监听 TCP。
- 不把 greetd socket、PAM worker 通道或未来 daemon IPC 暴露给前端。
- locker 用系统 UID/账户数据 API 获取当前真实 UID 和对应账户，不从主题、
  环境中的任意 username 或 `auth.begin` 参数推导目标身份。
- `ext-session-lock-v1` handle 只由 locker 主进程持有；任何下游结果都只能生成
  内部 `UnlockAuthorized`，不能直接解锁。
- 禁止外部导航、新窗口、下载和开发者工具。
- 默认禁止网络访问及远程资源。
- 使用严格 CSP。
- bridge 使用方法白名单和结构化反序列化。
- 限制用户名、回答和消息的最大长度。
- 防止重复提交和并发认证请求。
- 日志中不记录 PAM 回答、密码、token 或完整 IPC payload。
- `Debug`/`Display` 不泄露 secret。
- greeter 页面刷新、崩溃和 host 退出时取消活动 greetd session。
- locker 在取得 lock 后遇到 renderer、主题、controller 或 PAM worker 崩溃时必须
  fail closed，切换到可信 GTK fallback 并保持锁定。
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

- 通用主题选择和 greeter/locker 可选覆盖；值可以是稳定 ID 或绝对路径，入口仍属于
  `theme.toml`。
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
均拒绝未知字段，语法层只反序列化原始值，语义层再区分主题 ID/绝对路径并验证 ID、路径、
空值、数量与跨字段约束。
初始公开结构为：

```toml
[themes]
default = "nocturne"
greeter = "custom-greeter"
locker = "/srv/fomalhaut/themes/custom-locker"

[sessions]
wayland_dirs = ["/usr/local/share/wayland-sessions", "/usr/share/wayland-sessions"]
x11_dirs = ["/usr/local/share/xsessions", "/usr/share/xsessions"]
executable_search_paths = ["/usr/local/bin", "/usr/bin"]

[power]
actions = ["poweroff", "reboot", "suspend"]

[display]
scale = 1.5
```

需要让两个角色使用不同 WebKit zoom 时，`scale` 改用 TOML dotted-key table 形式：

```toml
[display]
scale.greeter = 1.5
scale.locker = 1.0
```

- `themes` 中的每个字段都是可选主题 ID 或绝对主题目录；选择顺序是角色专用、
  `default`、内嵌 minimal theme。ID 通过第 8 节固定的本地/系统搜索根和稳定冲突顺序解析为
  绝对目录，绝对路径继续支持开发与非标准 prefix。入口和协议版本由目录内必需的 `theme.toml`
  决定，避免配置与清单出现两个互相冲突的入口来源。已经移除的 `[frontend]` 不再属于配置
  schema，出现时按未知顶层字段拒绝。
- `sessions` 缺失时沿用固定默认目录。section 存在时，每个缺失字段仍继承对应默认值；显式
  空数组用于禁用该类目录。所有目录必须是无 NUL 的绝对路径，保持数组顺序作为优先级；
  至少要发现一个最终可用 session，否则启动失败。
- `power` 缺失时所有电源动作关闭。`actions` 是至多三个互不重复的枚举 allowlist，只接受
  `poweroff`、`reboot` 和 `suspend`；显式空数组等同关闭。配置顺序不影响 capability 的稳定
  顺序，宿主固定按 poweroff、reboot、suspend 排列，并与 logind 当前返回 `yes` 的动作求交集。
  这是运行时和非标准部署的 fail-closed 默认；标准源码安装器首次创建配置时显式写入全部三个
  动作，后续升级不改写既有电源策略。
- `display` 缺失时 greeter/locker 页面缩放倍率都为 `1.0`。`scale` 是严格的 untagged union：
  可以是同时应用于两个角色的单个有限浮点数，也可以是只包含 `greeter`、`locker` 且两者都
  必须出现的 table；dotted keys 与等价的 `[display.scale]` table 语法均由 TOML 解析器处理。
  标量与 table 不能混用，角色 table 不允许缺项或未知字段。每个倍率都必须在
  `0.5..=4.0`，语义校验后立即收敛为内部固定的 `greeter`/`locker` 两个值，不把 union 分支传播
  到 host。该倍率只应用于 WebKit `zoom-level`，不负责 Cage 光标大小，也不尝试从不可靠的 EDID
  物理尺寸自动推断 DPI。
- greeter 的独立 Cage 通常不继承桌面输出缩放，因此可使用 `1.5` 等显式页面 zoom；locker
  已运行在 niri 等现有 compositor 的逻辑坐标和输出缩放内，通常使用 `1.0`，避免把 compositor
  scale 与 WebKit zoom 叠加。推荐 systemd user unit 必须清除从 manager 环境继承的
  `GDK_SCALE`/`GDK_DPI_SCALE`，让角色配置成为唯一的额外页面缩放来源；直接运行二进制时，用户
  同样不得额外设置这两个变量。
- 首个切片不加入可配置网络、CSP、开发者工具或任意 header。安全策略仍是编译期拒绝式常量，
  避免把主题配置扩展成降低宿主边界的权限开关。
- 日志目标和记忆用户/session 继续留作后续字段；在实现前未知字段会被拒绝，不能
  提前依赖未承诺的配置键。

`fomalhaut-config` 语义校验后分别产生 `for_greeter()` 和 `for_locker()` 视图。locker
视图不包含 session discovery 命令或登录用户选择权能。PAM service 名称固定为
`fomalhaut-lock`，由安装包提供和系统管理员审核，不作为主题或普通前端可切换的
安全策略字段。

共享 `fomalhaut-config` 已实现 `[themes]` 与 `for_greeter()`/`for_locker()` 角色视图；
角色专用主题优先于 default，locker 视图不暴露 session discovery 或用户枚举配置。
`[frontend].path` 兼容已经移除，任何 `[frontend]` table 都由严格反序列化作为未知字段拒绝；
安装器 preflight 同样拒绝旧 table 并要求管理员显式迁移。配置和外部主题纵向切片已用自动化
测试验证：配置缺失时安全回退、角色主题优先级、旧 `[frontend]` 与其他未知字段拒绝、相对路径拒绝、
显示缩放边界、显式 session 优先级、64 KiB 上限、清单 protocol/入口校验、URI 语法、MIME 白名单、顶层导航
限制、配置根 symlink、根内相对 symlink、根外 symlink 拒绝以及资源读取边界。完整 workspace
测试同时继续覆盖真实 Unix socket greetd 流程；内嵌主题仍通过 Wayland/WebKitGTK 运行探针
验证，外部主题的真实系统安装步骤记录在 `docs/CONFIGURATION.md`。安装器隔离测试还覆盖
全新 `[themes].default`、旧字段 preflight 拒绝、角色覆盖/显示/电源策略保留和重复运行幂等。

无效安全配置应导致启动失败或回退到安全默认值，不能静默放宽限制。

## 13. 错误、日志和恢复

- 使用结构化错误类型区分配置、transport、协议、认证、session、PAM worker、
  session-lock 和 WebView 错误。
- 面向用户的错误信息不直接等于内部错误或 PAM description。
- 日志默认不包含用户名以外的认证内容；是否隐藏用户名可进一步配置。
- secret 类型必须提供安全的 `Debug` 实现。
- WebView 无法启动或主题加载失败时显示最小可信故障页面并保留可诊断日志；
  locker 已锁定后的故障页必须是 GTK 侧 fallback，不得因 renderer 已崩溃而不可用。
- greeter 无法连接 `GREETD_SOCK` 时明确退出，避免呈现一个永远无法登录的假界面。
- locker 在请求 lock 前无法建立 PAM worker 时明确退出；在 `locked` 后失去 worker
  时保持 lock 并提供可控重试，不解锁。
- 正常退出路径应显式等待活动认证 session 取消；panic/abort 等无法等待异步 IPC 的路径
  通过关闭 transport 触发连接级清理，不在 panic hook 中启动后台异步任务。

## 14. 测试策略

### 14.1 Core 单元测试

- 使用 fake `LoginBackend` 和 `ReauthBackend` 验证通用 auth state，并证明两个 trait
  不会互相泄漏启动 session 或替换身份的权能。
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

### 14.3 PAM 与 locker backend 测试

- 在不要求 CI 使用真实用户密码的前提下，用可控 PAM service/module 验证
  echo-on、echo-off、info、error、MFA、重试和 `PAM_MAXTRIES`。
- 验证 PAM worker IPC 长度边界、取消、secret 脱敏/清理、worker 崩溃和可控重建。
- 验证 `ReauthBackend::begin_reauth()` 始终使用当前 UID/session 身份，拒绝任何
  username 替换路径，且从不产生 `SessionCommand`。

### 14.4 Session 测试

- X11 和 Wayland desktop entry。
- `Hidden`、`NoDisplay` 和重复项。
- 无效或缺失的 `Exec`。
- session ID 稳定性。
- 路径穿越、符号链接和目录优先级。

### 14.5 Frontend protocol 测试

- JSON schema 和 Rust 类型一致。
- 协议版本不兼容。
- 未知方法和字段。
- 消息长度限制。
- 并发、重复和乱序请求。
- 前端永远无法提供实际 session command。
- `mode` 判别快照、角色 capability、两种 `auth.begin` 参数和越权方法拒绝。
- 多视图 bootstrap、sequence watermark、刷新 epoch 与乱序/重复事件丢弃。
- 单一通用主题、greeter/locker 分开主题与新旧配置冲突。

### 14.6 WebView 与 session-lock 集成测试

- 自定义 scheme 和 MIME 类型。
- 外部导航、弹窗和下载被阻止。
- 默认网络访问被阻止。
- CSP 生效。
- 主题目录逃逸被拒绝。
- 页面刷新触发认证取消和状态重同步。
- Cage 下启动、登录和退出。
- `ext-session-lock-v1` 的 `monitor`、`locked`、`failed`、`unlocked` 与只有授权后解锁的顺序。
- 多输出、热插拔、缩放变化、每输出 WebView 初始化和页面内弹层。
- renderer 崩溃、主题损坏和 PAM worker 崩溃时切换到可信 GTK fallback，不释放 lock。
- suspend 前 readiness 时序、立即终止 locker 的 fail-closed 行为，以及 niri 和至少
  另一个目标 compositor 的实机验证。

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
- backend-neutral core 重构与新增 `fomalhaut-greetd`、`fomalhaut-pam`、`fomalhaut-config`、
  `fomalhaut-gtk` 时，必须为所有对外 API/依赖边发生变化的已发布 crate 建立联合
  changeset，并同步 Semifold package 列表与 CI。
- 在 alpha 阶段将 v1 更新为 greeter/locker 判别协议时，同一发布事务必须覆盖
  `fomalhaut-web`、`fomalhaut-sdk`、内嵌 minimal theme、React 参考主题与两个宿主。
- 同一 host 至少支持其当前协议版本。
- 破坏性前端协议变更必须增加主版本。
- 新增可选字段不应破坏旧主题。
- 主题清单声明所需协议版本；不兼容时显示清晰的故障页面。
- greetd IPC 兼容范围由 `greetd_ipc` 依赖版本和集成测试共同定义。
- 未来 `fomalhautd` 使用独立、鉴权的 daemon IPC，不复用 WebView JSON bridge
  作为特权协议；其版本和 greetd 迁移承诺需要单独设计。

## 17. 待原型验证的决策

以下内容在完成小型原型后再固化：

- GTK4 + WebKitGTK WebView 在 `gtk4-session-lock` 多输出窗口中的实机可行性，
  包括 hotplug、scale、renderer 崩溃 fallback 和 unlock/GDK roundtrip。
- `pam-client 0.5.0` 一次性 worker 的进程管理、有界 IPC 与取消/超时终止已经实现并由 fake
  worker 验证；PAM service/module fixture 的真实可移植性仍须验证，且不得削弱 4.3 节的
  fail-closed 边界。
- 自定义 scheme 的基本 CSP 与 MIME 行为已验证；仍需验证目标发行版与
  WebKitGTK 版本矩阵，但不再把基本可行性列为未知。
- renderer sandbox 在不同发行版中的默认状态和配置方法。
- 首个正式 locker 目标 compositor 矩阵；实际部署环境 niri 必须包含在内，并至少选另一个
  `ext-session-lock-v1` compositor 完成回归。
- WebKitGTK、Cage、greetd 和 `gtk4-layer-shell` 的最低兼容版本；Rust 工具链继续跟随
  stable。当前开发依赖
  跟随滚动最新稳定版本，最低兼容版本只能在发行版验证后声明。
- `fomalhautd` 的特权模型、daemon IPC、peer credential/logind 映射、seat/VT 与
  session 监督/恢复语义。这些未完成前不开始替换 greetd。

这些决策不得削弱本文定义的 core/UI 分离和前端权限边界。
