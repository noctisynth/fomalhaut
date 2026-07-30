# Fomalhaut

Fomalhaut（北落师门）是一个基于 greetd、使用本地 WebView 渲染界面的登录 greeter。

项目将 greetd IPC 和认证状态机封装为与 UI 无关的 Rust core，并通过受限、版本化的消息
协议连接用户自行提供的 HTML、CSS 和 JavaScript 前端。Fomalhaut 不实现 PAM，也不向前端
开放 greetd socket、任意命令执行或远程登录服务。

> Fomalhaut 目前处于初始化阶段，尚不能用于实际登录。

## Workspace

- `fomalhaut-core`：greetd IPC 和认证状态机。
- `fomalhaut-session`：可信 desktop session 发现与解析。
- `fomalhaut-web`：WebView、主题资源和前端协议 bridge。
- `fomalhaut`：组合各组件的最终 greeter 程序。

详细技术方案见 [`docs/DESIGN.md`](docs/DESIGN.md)，实现进度见 [`TODO.md`](TODO.md)。

## 开发

项目使用 Rust 2024 Edition，并跟随最新 Rust stable 滚动更新，不维护固定 MSRV。
第三方依赖也持续跟随最新稳定版本，并通过提交的 `Cargo.lock` 保持单个提交可复现。

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings -D clippy::unwrap_used
cargo test --workspace --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Monorepo 的 changeset、独立包版本和发布由 Semifold 管理：

```sh
smif status
smif commit
```

所有包当前使用 `alpha` release channel。`smif version` 和 `smif publish` 不得在本地
执行；版本更新和发布由 GitHub Actions 中的 Semifold CI 独占处理。

开始贡献前请阅读 [`AGENTS.md`](AGENTS.md) 和 [`CONTRIBUTING.md`](CONTRIBUTING.md)。

## 许可证

Fomalhaut 仅依据 GNU Affero General Public License version 3
（`AGPL-3.0-only`）授权。完整条款见 [`LICENSE`](LICENSE)。
