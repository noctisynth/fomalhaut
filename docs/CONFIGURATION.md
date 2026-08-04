# Fomalhaut 配置与外部主题

Fomalhaut 固定读取 `/etc/fomalhaut/config.toml`。文件不存在时使用内嵌 minimal theme 和系统
session 默认目录；文件存在但无法读取、包含未知字段或验证失败时，Fomalhaut 以非零状态退出。

## 最小外部主题配置

```toml
[frontend]
path = "/etc/fomalhaut/themes/my-theme"
```

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

完整请求、响应、事件和长度约束见仓库根目录的 `protocol/v1.schema.json`。实现登录界面至少需要
处理 `state.get`、`session.select`、`auth.begin`、`auth.respond`、`auth.cancel` 以及所有
`auth.*`/`session.*` 事件。主题可以读取其页面中输入的认证内容，因此管理员仍应只安装可信
来源的主题。

仓库中的 React 参考主题可通过以下命令构建：

```sh
bun run build:theme
```

构建产物位于 `packages/fomalhaut-theme/dist`。在本地仓库测试时，可将其绝对路径直接写入：

```toml
[frontend]
path = "/home/example/Projects/fomalhaut/packages/fomalhaut-theme/dist"
```

该目录必须允许 `greeter` 用户遍历和读取。普通浏览器开发服务器会使用仅开发环境存在的模拟
transport；它接受 `fomalhaut` 作为 fixture 密码。生产构建不会包含该 transport，脱离真实
WebKit bridge 时会拒绝登录请求。

`state.get` 的 `users` 数组提供经过宿主过滤的 `username`、`displayName` 和可选
`avatarUrl`。头像 URL 只会是宿主管理的 `fomalhaut://avatar/<id>`，主题不得尝试从用户名推导
本地路径。用户列表可能为空，主题必须始终保留手工用户名输入；选择用户摘要后仍应将其
`username` 传给 `auth.begin`，最终认证结果只由 greetd/PAM 决定。

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
