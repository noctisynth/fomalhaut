# Fomalhaut

[English](README.md)

Fomalhaut（北落师门）是一个基于
[greetd](https://git.sr.ht/~kennylevinsen/greetd)、使用 WebKitGTK 在本地渲染界面的
Linux 登录 greeter。它让登录界面可以完全由 HTML、CSS 和 JavaScript 定制，同时仍由 greetd
负责认证和用户会话管理。

Fomalhaut 不是 Web 服务器，也不实现 PAM。Rust 宿主负责与 greetd 通信、发现可信桌面会话，
并通过精简且版本化的协议连接本地主题。主题无法获得 greetd socket、会话命令或任意进程执行
能力。

> [!IMPORTANT]
> Fomalhaut 目前处于 alpha 阶段。greetd、Cage 和 Wayland 登录链路已经在真实系统中验证，
> 但配置格式、主题 API 和打包方式仍可能在首个稳定版本前发生变化。

## 效果预览

| 用户选择 | 身份认证 |
| :---: | :---: |
| ![Nocturne 用户选择界面](docs/assets/nocturne-user-selection.png) | ![Nocturne 身份认证界面](docs/assets/nocturne-authentication.png) |

## 项目效果

- 提供完整的 Nocturne 参考主题和内嵌的最小主题。
- 支持用户发现、头像、手工用户名输入和可信桌面会话选择。
- 支持多轮 PAM 认证，包括密码、明文输入和任意后续提示。
- 通过 systemd-logind 提供可选、受策略限制的电源操作。
- 支持小数显示缩放，并允许主题替换完整登录体验。
- 使用受限的本地 WebView，默认禁用远程资源、任意导航、下载、弹窗和开发者工具。

主题可以读取用户在其页面中输入的认证信息，因此只应安装来源可信且已经审查的主题。

## 安装

目前支持的安装方式是仓库自带的源码安装器。它会构建 greeter 和 Nocturne 主题，完成系统安装，
并更新 Fomalhaut 与 greetd 配置。

安装前需要准备：

- 已安装 greetd、Cage、D-Bus、GTK 4 和 WebKitGTK 6.0 的 Linux 系统；
- 最新 stable Rust 工具链及 Cargo；
- Bun canary 和 Git；
- 一个可以通过 `sudo` 完成系统安装的普通用户账户。

在 Arch Linux 上，安装器可以通过 `paru`、`yay` 或 `pacman` 安装缺少的系统软件包，但 Rust
和 Bun 仍需单独安装。其他发行版需要先自行安装等价的构建与运行时依赖。

```sh
git clone https://github.com/noctisynth/fomalhaut.git
cd fomalhaut
```

> [!IMPORTANT]
> 安装时应显式设置 `--display-scale`。Fomalhaut 运行在独立的 Cage 会话中，不会继承桌面
> 环境的缩放设置。现在许多笔记本和 HiDPI 显示器需要使用 `1.5` 或 `2.0`，无需缩放的显示器
> 通常使用 `1.0`。允许的范围为 `0.5` 到 `4.0`。

### 全新安装

使用适合当前显示器的缩放倍率构建并安装 Fomalhaut：

```sh
./install.sh --display-scale 1.5
```

安装器不会自动启用 greetd。请从文本控制台执行以下命令；如果当前仍在图形会话中，请先保存
工作：

```sh
sudo systemctl enable --now greetd.service
```

### 从其他显示管理器迁移

先安装 Fomalhaut，不要使用 `--restart`：

```sh
./install.sh --display-scale 1.5
```

保存当前工作并切换到文本控制台，确认正在使用的显示管理器，先禁用并停止该服务，然后才能
启用 greetd。以下示例从 SDDM 迁移：

```sh
systemctl status display-manager.service
sudo systemctl disable --now sddm.service
sudo systemctl enable --now greetd.service
```

请将 `sddm.service` 替换为实际使用的服务，例如 `gdm.service` 或 `lightdm.service`。不要同时
启用两个显示管理器。

默认安装以下内容：

- `/usr/local/bin/fomalhaut`
- `/etc/fomalhaut/themes/nocturne`
- `/etc/fomalhaut/config.toml`
- `/etc/greetd/config.toml`

安装器不会检查或修改显示管理器服务的启用状态。`--restart` 只适用于已经使用 greetd 的系统
更新：它会重启 greetd，但不会启用该服务。

```sh
./install.sh --display-scale 1.5 --restart
```

运行 `./install.sh --help` 可以查看显示缩放、光标大小、greeter 用户、安装前缀和隔离目录等
选项。完整的安装与升级行为见[配置与安装文档](docs/CONFIGURATION.md)。

## 使用 Fomalhaut

greetd 启动 Fomalhaut 后，可以选择发现的用户或使用手工登录，选择可用的桌面会话，并依次回答
系统 PAM 配置给出的认证提示。认证成功后，Fomalhaut 将会话交还给 greetd，并在桌面启动后退出。

系统配置文件位于 `/etc/fomalhaut/config.toml`，可以设置主题、显示缩放、用户 provider、会话
搜索目录和可选电源操作。配置格式、greetd/Cage 示例和外部主题用法见
[配置文档](docs/CONFIGURATION.md)。

## Workspace

Workspace 结构、组件职责、协议和安全边界见[技术设计](docs/DESIGN.md)。

## 开发

开发环境、检查命令和 Pull Request 流程见[贡献指南](CONTRIBUTING.md)。

## 许可证

Fomalhaut 仅依据 [GNU Affero General Public License v3.0](LICENSE)
（`AGPL-3.0-only`）授权。
