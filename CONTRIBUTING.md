# 贡献指南

感谢你对 Fomalhaut 的关注。

## 开始之前

1. 阅读 [`docs/DESIGN.md`](docs/DESIGN.md)，了解当前技术方案和安全边界。
2. 阅读 [`TODO.md`](TODO.md)，确认工作是否已经列入计划。
3. 阅读 [`AGENTS.md`](AGENTS.md)，遵守仓库的实施、错误处理和 Cargo manifest 约定。

如果实现需要改变架构、协议、安全边界、兼容性或依赖选择，应先更新 `docs/DESIGN.md`，
再更新 `TODO.md`，然后再实现代码。

## 本地检查

提交变更前请运行：

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings -D clippy::unwrap_used
cargo test --workspace --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

生产 Rust 代码不得使用可能 panic 的 `unwrap()`。可恢复失败必须通过调用方可以处理的错误
传播。

## Cargo manifest

除初始化虚拟 workspace 时已经批准的根 manifest 外，不要手工编辑 Cargo manifest。
依赖和 crate 变更应使用 `cargo add`、`cargo remove`、`cargo new` 或 `cargo init` 完成。

各 crate 独立维护版本。影响可发布 crate 的变更应使用 `smif commit` 创建 changeset，
不要手工修改 package version，也不要在本地执行 `smif version` 或 `smif publish`。
版本更新和发布仅由 GitHub Actions 中的 Semifold CI 执行。

## 提交

- 保持每个提交的目的单一。
- 在提交说明中解释行为变化及原因。
- 同时提交实现所需的测试和文档。
- 不要提交构建产物或敏感信息。
