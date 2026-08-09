# Fomalhaut 配置与外部主题

Fomalhaut 固定读取 `/etc/fomalhaut/config.toml`。文件不存在时使用内嵌 minimal theme 和系统
session 默认目录；文件存在但无法读取、包含未知字段或验证失败时，Fomalhaut 以非零状态退出。

## 从源码安装

仓库根目录的安装器会锁定依赖构建 release 二进制和 React 参考主题，并安装到系统目录：

```sh
./install.sh --greeter-scale 1.5 --locker-scale 1.0
```

### 显示缩放

两个角色确实使用相同页面 zoom 时，可以传入 `--display-scale 1.5`。greeter 运行在独立 Cage
中、locker 运行在 niri 等现有 compositor 中时，通常应改用成对的
`--greeter-scale`/`--locker-scale`：Cage 不继承桌面输出缩放，而 locker 已由现有 compositor
处理逻辑坐标和输出 scale。共享参数不能与角色参数混用，两个角色参数也不能只提供一个；全部
倍率允许范围均为 `0.5` 到 `4.0`。

如果首次安装时省略该选项，新配置会使用 `1.0`；更新安装时省略则保留已有配置值。光标大小
由独立的 `--cursor-size` 控制。

首次创建 `/etc/fomalhaut/config.toml` 时，源码安装器还会默认允许 poweroff、reboot 和 suspend；
更新安装不会新增、覆盖或扩大既有配置中的电源策略。

### 全新安装

使用适合显示器的缩放倍率运行安装器：

```sh
./install.sh --greeter-scale 1.5 --locker-scale 1.0
```

安装器不会自动启用 greetd。请从文本控制台执行以下命令；如果当前仍在图形会话中，请先保存
工作：

```sh
sudo systemctl enable --now greetd.service
```

### 从其他显示管理器迁移

先运行安装器，不要使用 `--restart`：

```sh
./install.sh --greeter-scale 1.5 --locker-scale 1.0
```

保存工作并切换到文本控制台。通过 `display-manager.service` 确认当前服务，先禁用并停止原显示
管理器，然后再启用 greetd。以下示例从 SDDM 迁移：

```sh
systemctl status display-manager.service
sudo systemctl disable --now sddm.service
sudo systemctl enable --now greetd.service
```

请将 `sddm.service` 替换为实际服务，例如 `gdm.service` 或 `lightdm.service`。不要同时启用两个
显示管理器。

脚本必须由普通用户执行；构建阶段不使用 root，只在写系统目录时调用 `sudo`。默认安装结果为：

- `/usr/local/bin/fomalhaut`
- `/usr/local/bin/fomalhaut-lock`
- `/usr/local/lib/systemd/user/fomalhaut-lock.service`
- `/usr/local/share/doc/fomalhaut-lock/niri.kdl`
- `/usr/local/share/doc/fomalhaut-lock/swayidle.conf`
- `/etc/pam.d/fomalhaut-lock`
- `/etc/fomalhaut/themes/nocturne`
- `/etc/fomalhaut/config.toml`
- `/etc/greetd/config.toml`

源码构建所需的 Rust、Bun canary、GTK/WebKitGTK 开发环境必须已经可用；真实系统安装还会在写入
前确认 `/usr/bin/cage`、`/usr/bin/dbus-run-session` 和指定的 greeter 账户存在。

在 Arch Linux 上，安装器会先用 `pacman -T` 检查缺失的构建和运行包，并优先调用 `paru`，
不存在时依次回退到 `yay` 和 `sudo pacman`。包安装保留交互确认且不会触发隐式全系统升级；
Rust 与 Bun 工具链由用户自行提供，安装器不会通过系统包管理器安装、升级或检查其发布通道。
AccountsService 是可选的用户资料增强，不会被强制安装。`--system-root` 测试模式从不修改
宿主机的软件包状态。

重复运行同一命令即可更新安装。既有二进制和普通主题目录会保留带时间戳的备份，主题 release
不会自动删除。两个 TOML 文件会先解析和验证，只更新安装器负责的字段，并在同目录保留
`*.bak.<时间戳>.<进程号>`；无效 TOML、重复目标字段或 symlink 配置会让安装器拒绝修改。
配置 preflight 在切换二进制和主题之前执行，因此可提前检测的问题不会造成部分安装。
已有的 `/etc/pam.d/fomalhaut-lock` 与安装器提供的策略不同时会作为管理员策略保留并给出警告，
不会在升级时被静默覆盖。若构建后的二进制、systemd unit、完整主题树和生成的配置文本都与
当前安装相同，重复执行不会创建任何新备份、
主题 release 或配置替换；只有对应内容确实变化时才生成备份并原子切换。
交互终端中，安装器使用颜色区分步骤、成功、未变化、备份和错误；设置 `NO_COLOR=1` 可以禁用
颜色。输出被重定向或通过管道处理时会自动使用纯文本。
安装器不检查 greetd 是否已经设为开机启动，也不检测或禁用其他显示管理器。`--restart` 只会
执行 `systemctl restart greetd`，适用于已经使用 greetd 的系统更新，不会为全新安装启用服务。
确认可以退出当前会话并且 greetd 已经启用时，可执行：

```sh
./install.sh --greeter-scale 1.5 --locker-scale 1.0 --restart
```

使用非默认安装前缀时传入绝对 `--prefix`。`--system-root /tmp/fomalhaut-root` 只用于在隔离目录
验证安装结果，不调用 `sudo`，也不允许 `--restart`。完整参数见 `./install.sh --help`。

## Wayland locker、PAM 与 readiness

`fomalhaut-lock` 只能在已登录用户自己的 Wayland session 中运行，并要求 compositor 广告
`ext-session-lock-v1`。不支持该协议、X11 或限制第三方 locker 的桌面环境会在请求 lock 前
明确失败，不会回退到普通全屏或 layer-shell 伪锁屏。Arch 安装还需要 `pam` 与
`gtk4-layer-shell` 1.2 或更高版本；源码安装器会检查并安装缺失的发行版包。

locker 要求进程的真实、有效、保存和 filesystem UID 一致且非 root，再通过 NSS 固定当前身份；
主题不能提交用户名或切换用户。每次
认证使用独立的一次性 PAM 子进程，固定读取 `/etc/pam.d/fomalhaut-lock`。安装器首次安装的
Arch 策略为：

```pam
auth      include  system-auth
account   include  system-auth
```

管理员应按发行版和本地认证要求审查该文件。后续源码安装发现已有不同策略时只会保留并警告，
不会自动覆盖。PAM worker、renderer、主题或 controller 在已经锁定后失败时，locker 继续持有
session lock，并切换到可信 GTK 故障/重试界面；取消认证也不会解锁。密码错误、账户策略拒绝和
达到尝试次数属于普通 PAM 拒绝，继续由当前 Web 主题展示 `auth.failed` 并允许重试，不进入原生
故障页。

源码安装器提供 `Type=notify` 用户服务。安装或更新后先让当前用户的 systemd manager 重读 unit：

```sh
systemctl --user daemon-reload
systemctl --user start fomalhaut-lock.service
```

第二条命令只在 compositor 已发出 `locked`、controller 已记录 `lock.acquired` 且 locker 发送
`READY=1` 后返回；认证解锁后进程正常退出。服务不需要 enable，它由锁屏触发器按需启动。
直接执行 `fomalhaut-lock` 时进程同样保持前台，但没有 systemd readiness 消费者。

该 user unit 显式使用 `NoNewPrivileges=no`。这是 PAM 兼容边界，不表示 Fomalhaut 自身以 root
运行或带有 setuid bit：Arch 的 `pam_unix` 需要透明执行系统安装的 setuid `unix_chkpwd` helper，
`NoNewPrivileges=yes` 会阻止该 helper 获得校验受保护密码数据库所需的身份。unit 仍保留
`LockPersonality=yes` 与 `RestrictSUIDSGID=yes`，后者禁止创建新的 SUID/SGID 文件。管理员若替换
PAM stack，应同时复核这一权限要求，不能在未验证真实密码认证的情况下自行启用
`NoNewPrivileges`。

unit 同时使用 `UnsetEnvironment=GDK_SCALE GDK_DPI_SCALE`，只为 locker 服务清除 user manager
可能继承的工具包缩放变量。niri/GTK 仍负责输出缩放，额外页面 zoom 只由 locker 的配置值决定。
直接执行二进制进行调试时也应先确认没有设置这两个变量；正常使用优先通过上述 systemd user
service 启动。

niri 用户可以把安装到 `/usr/local/share/doc/fomalhaut-lock/niri.kdl` 的两段配置合并进
`~/.config/niri/config.kdl`。顶层启动项负责 idle/挂起集成：

```kdl
spawn-at-startup "swayidle" "-w" "timeout" "300" "systemctl --user start fomalhaut-lock.service" "lock" "systemctl --user start fomalhaut-lock.service" "before-sleep" "systemctl --user start fomalhaut-lock.service"
```

在现有 `binds` block 中加入手工锁屏快捷键：

```kdl
Super+Alt+L hotkey-overlay-title="Lock the Screen: Fomalhaut" { spawn "systemctl" "--user" "start" "fomalhaut-lock.service"; }
```

`swayidle` 虽然名称来自 Sway，但它通过标准 Wayland idle/session 协议工作，也可在 niri 等其他
compositor 中使用。安装到 `/usr/local/share/doc/fomalhaut-lock/swayidle.conf` 的独立命令示例为：

```sh
swayidle -w \
    timeout 300 'systemctl --user start fomalhaut-lock.service' \
    lock 'systemctl --user start fomalhaut-lock.service' \
    before-sleep 'systemctl --user start fomalhaut-lock.service'
```

`before-sleep` 必须保留阻塞的 `systemctl start`，确保 suspend 只在 readiness 之后继续。当前自动化
测试覆盖 readiness datagram、角色协议、PAM worker fail-closed 状态机和多视图事件路由；niri
及另一种 compositor 的真实多输出、热插拔、挂起和 PAM module 组合仍需在目标系统验证。

## 外部主题配置

```toml
[themes]
default = "/etc/fomalhaut/themes/my-theme"
```

`default` 主题同时供 greeter 和 locker 使用。管理员也可以只覆盖其中一个角色；选择优先级固定为
“角色专用 → `default` → 内嵌 minimal theme”：

```toml
[themes]
default = "/etc/fomalhaut/themes/nocturne"
greeter = "/etc/fomalhaut/themes/custom-greeter"
locker = "/etc/fomalhaut/themes/custom-locker"
```

每个字段都必须是绝对路径。每个主题仍只有一个 `theme.toml` entrypoint；同一页面通过 SDK
提供的运行模式呈现 greeter 或 locker。两个宿主都使用上述主题选择；配置两个角色路径用于
允许管理员选择两套独立主题。

迁移期仍接受单独出现的旧 `[frontend].path`，greeter 会输出弃用提示。`[frontend]` 与
`[themes]` 同时出现时配置无效，避免隐式选择。源码安装器更新旧配置时会把
`[frontend].path` 迁移为 `[themes].default`，保留已有的 `greeter`/`locker` 覆盖和其他
管理员配置；若旧 `[frontend]` 包含未知键，安装器会拒绝删除该 table。

主题目录必须是绝对路径，并包含 `theme.toml`：

```toml
[theme]
name = "My Theme"
protocol = 1
entrypoint = "index.html"
```

入口和其他资源都通过 `fomalhaut://theme/` 加载。HTML 中应使用相对 URL，例如：

```html
<link rel="stylesheet" href="style.css">
<script src="app.js"></script>
```

主题不能使用内联脚本、远程 URL、网络请求、frame、object 或表单导航。资源路径只支持 ASCII
字母、数字、`-`、`_`、`.` 和 `/`；不支持 percent-encoded 文件名。顶层页面只有清单入口，
其他 HTML 不能作为导航目标。

前端通过以下入口发送协议 v1 请求：

```js
const response = await window.webkit.messageHandlers.fomalhaut.postMessage({
  protocol: 1,
  id: 1,
  method: 'state.get',
  params: {},
});

window.addEventListener('fomalhaut:event', (event) => {
  console.log(event.detail);
});
```

完整请求、响应、事件和长度约束见仓库根目录的 `protocol/v1.schema.json`。`state.get` 返回以
`mode: "greeter" | "locker"` 判别的联合快照，并携带已经发布的最后一个 event `sequence`。
同一主题入口应先按 `mode` 收窄：greeter 处理用户与 session 选择，并以
`auth.begin(username)` 开始认证；locker 只展示宿主固定的 `identity`，不提供用户/session
切换，并以无参数 `auth.begin()` 重新认证当前用户。两者共享 `auth.respond`、`auth.cancel`、
prompt/message 和公共认证状态；`session.*` 只属于 greeter，`lock.*` 只属于 locker。主题可以
读取其页面中输入的认证内容，因此管理员仍应只安装可信来源的主题，并在 SDK 调用前同步清空
输入元素。

推荐主题直接使用同一个 `fomalhaut-sdk`。factory 会先完成 bootstrap，并返回可由只读 `mode`
字段收窄的泛型 client：

```ts
const client = await createFomalhautClient();

if (client.mode === "greeter") {
  await client.auth.begin("alice");
  await client.session.select("wayland:sway");
} else {
  await client.auth.begin();
  // client.session 在这里由 TypeScript 收敛为 undefined。
}
```

仓库中的 React 参考主题可通过以下命令构建：

```sh
bun run build:theme
```

构建产物位于 `packages/fomalhaut-theme/dist`。在本地仓库测试时，可将其绝对路径直接写入：

```toml
[themes]
default = "/home/example/Projects/fomalhaut/packages/fomalhaut-theme/dist"
```

该目录必须允许 `greeter` 用户遍历和读取。普通浏览器开发服务器会使用仅开发环境存在的模拟
transport；它接受 `fomalhaut` 作为 fixture 密码。生产构建不会包含该 transport，脱离真实
WebKit bridge 时会拒绝登录请求。

`state.get` greeter 分支的 `users` 数组提供经过宿主过滤的 `username`、`displayName` 和可选
`avatarUrl`。头像 URL 只会是宿主管理的 `fomalhaut://avatar/<id>`，主题不得尝试从用户名推导
本地路径。用户列表可能为空，主题必须始终保留手工用户名输入；选择用户摘要后仍应将其
`username` 传给 `auth.begin`，最终认证结果只由 greetd/PAM 决定。locker 分支只提供当前可信
`identity` 的同形显示字段，不提供 UID 或账户枚举，也不允许主题改变认证目标。

## 电源管理

Fomalhaut 的运行时配置在缺少 `[power]` 时默认关闭全部电源操作。源码安装器首次创建配置时会
显式允许固定枚举中的全部三个动作：

```toml
[power]
actions = ["poweroff", "reboot", "suspend"]
```

数组只接受上述三个值且不得重复；显式空数组关闭全部动作。对已有配置执行更新安装时，缺失的
`[power]`、显式空数组和自定义 allowlist 都会原样保留。Fomalhaut 会通过系统 D-Bus 查询
systemd-logind，`state.get` 的 `capabilities.power` 只包含同时出现在配置中且对应 `Can*` 方法
返回 `yes` 的动作。`no`、`na`、`challenge` 或 logind 不可用都不会向主题发布能力，因此
greeter 和 locker 都不依赖 Polkit 交互 agent。

主题只能用 `power.request` 请求 capability 中存在的枚举动作。greeter 会先取消进行中的 greetd
认证，locker 会先取消当前 PAM transaction，再调用共享 backend 的非交互 `PowerOff(false)`、
`Reboot(false)` 或 `Suspend(false)`；不会执行 shell 命令或回退到 `systemctl`。locker 的电源请求
不会授权解锁或释放 session lock，suspend/resume 后仍保持锁定。发行版的 Polkit/logind 策略仍
须允许对应运行用户执行动作，否则该动作不会显示或会返回脱敏错误。

## 页面缩放

独立的 Cage greeter 会话不会继承 KDE/GNOME 的显示缩放配置。可以为 WebKit 页面显式设置支持
小数的缩放倍率：

```toml
[display]
scale = 1.5
```

该标量同时应用于 greeter 和 locker。需要分别设置时使用 dotted keys，并且两个角色必须同时
出现：

```toml
[display]
scale.greeter = 1.5
scale.locker = 1.0
```

配置缺失时两者默认均为 `1.0`，每个值允许范围为 `0.5` 到 `4.0`。倍率通过 WebKit 的
`zoom-level` 缩放整个主题，不会修改主题代码，也不影响 Cage 绘制的鼠标光标。光标大小可在
greetd 命令中独立设置，例如：

```toml
command = "dbus-run-session env XCURSOR_SIZE=48 cage -s -m last -d -- /usr/bin/fomalhaut"
```

配置 `[display].scale` 后不应再设置 `GDK_SCALE` 或 `GDK_DPI_SCALE`，以免工具包缩放与页面 zoom
叠加。对于独立 Cage greeter 和使用 `150%` 输出缩放的 niri locker，通常分别配置 `1.5` 与
`1.0`。

## 用户发现

默认配置等价于：

```toml
[users]
provider = "auto"
```

可选值如下：

- `auto`：优先读取 AccountsService；服务整体不可用时通过固定的 `/usr/bin/getent passwd`
  查询 NSS。AccountsService 明确拒绝访问、成功返回空列表或仅有无效条目时不会绕过其结果。
- `accounts_service`：只读取 AccountsService，不执行 NSS fallback。
- `nss`：只通过受限的 `getent passwd` 子进程读取 NSS。
- `none`：完全禁用用户枚举。

NSS 结果按照 `/etc/login.defs` 中的 `UID_MIN`/`UID_MAX` 和 login shell 过滤。AccountsService
头像只有通过文件类型、所有权/可信目录、大小和 PNG/JPEG/WebP 内容检查后才会公开。任何发现
失败都只产生空用户列表，不会阻止 greeter 启动或手工登录。需要显示名和头像时，应安装并启用
AccountsService；NSS fallback 只提供用户名。

## Session 搜索目录

可以覆盖 Wayland、X11 和相对 `TryExec` 的搜索目录：

```toml
[sessions]
wayland_dirs = [
  "/usr/local/share/wayland-sessions",
  "/usr/share/wayland-sessions",
]
x11_dirs = [
  "/usr/local/share/xsessions",
  "/usr/share/xsessions",
]
executable_search_paths = ["/usr/local/bin", "/usr/bin"]
```

目录顺序就是优先级。缺失字段继承上述默认值；显式空数组禁用对应目录类型。所有路径必须是
绝对路径。最终没有发现任何安全有效的 session 时，Fomalhaut 拒绝启动。

## greetd/Cage 示例

```toml
[terminal]
vt = 1

[default_session]
command = "dbus-run-session cage -s -m last -d -- /usr/local/bin/fomalhaut"
user = "greeter"
```

二进制、配置和主题目录必须允许 `greeter` 用户遍历和读取。Fomalhaut 不应以 root 运行。
