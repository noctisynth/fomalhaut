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
