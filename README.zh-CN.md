<p align="center"><img src="assets/app-icon.png" width="96" alt="ContainerHub 图标"></p>

# ContainerHub

基于 Rust 和 GPUI 的原生 macOS 容器工作台。管理 Apple container 和 Docker
环境，查看 Compose / Swarm Stack；桌面、Agent 和 Node.js 复用同一套管理模块。

[English](README.md) · [架构](docs/ARCHITECTURE.md) · [Node SDK](packages/node/README.md) · [参与开发](CONTRIBUTING.md)

**项目处于早期开发阶段。** 当前桌面界面为简体中文，尚无签名公证的正式安装包或
npm 发行包。Kubernetes、完整 Web 界面和生产环境远程注册尚未完成。

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/containers-dark.png">
  <img src="docs/images/containers-light.png" alt="ContainerHub 原生容器列表和详情">
</picture>

*截图来自真实 GPUI/Metal 渲染，使用隔离示例数据，不代表真实主机负载。*

## 当前功能

- 容器列表、搜索、排序、分页、批量启停、详情和任务历史。
- 按运行时能力提供日志、资源采样、高级创建、镜像管理和端口重建。
- Docker context、Compose 项目、Swarm 服务与副本数管理，以及容器快速概览。
- 环境添加、编辑、切换，亮暗色主题和适应窗口大小的列表/详情布局。
- 公共 Rust 模块、Node ESM/TypeScript SDK，以及直连/出站中继的加密 Agent。

| 范围 | 当前边界 |
| --- | --- |
| Apple container | 支持读取、日志、采样、镜像拉取/删除、高级创建、启停和端口重建；写入仅开放已适配的 CLI 1.6.0 契约。 |
| Docker | 支持资源清单、检查、基础创建和启停；日志/采样、镜像写入和端口重建尚未实现。 |
| Stack | 管理现有 Compose / Swarm 部署；尚不支持 YAML 部署、Compose up/down 或 Swarm 集群创建。 |
| 远程 | 已实现身份、授权、加密传输和恢复，并通过隔离本机测试；正式应用证明、凭据签发及真实公网/NAT 验收未完成。 |
| Web / Kubernetes | 已有浏览器协议模块；完整 Web 产品、云端同步及 Kubernetes 适配未完成。 |

运行中不等于健康，缺少采样不显示为零。实际验证范围见
[开发记录](docs/DEVELOPMENT.md) 和 [远程说明](docs/REMOTE-DEVELOPMENT.md)。

## 本机运行

桌面目前面向 **Apple silicon、macOS 26+**。需要 Rust **1.99.1+**、Xcode Command
Line Tools；打包脚本需要 Python 3，Node SDK 和相关测试需要 Node **22+**。
运行时 CLI 需单独安装。

```sh
git clone https://github.com/backrunner/containerhub.git
cd containerhub

# 显式示例模式，不操作本机容器
cargo run -p containerhub --locked -- --demo

# 连接当前用户的 Apple container
cargo run -p containerhub --locked

python3 scripts/package-dev.py   # dist/ContainerHub.app，本地 ad-hoc 签名开发包
python3 scripts/package-node.py  # dist/node，SDK 与当前平台 bridge
```

已有 Docker context 可以从环境管理添加。应用不会自动安装或升级运行时；未知
Apple CLI 版本保留已核验的读取能力，未验证的写入禁用。Agent 和浏览器模块的
构建、授权步骤见 [远程开发说明](docs/REMOTE-DEVELOPMENT.md)。

## 开发验证

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo build -p containerhub-bridge --locked
node tests/node-management.mjs
node tests/node-sdk-failures.mjs
cargo run -p containerhub --features ui-validation --locked -- --validate-ui
```

原生 UI 检查覆盖三种窗口尺寸和亮暗色；`--structure-only` 可跳过 Metal 图片输出。
图形渲染需要 macOS 图形会话。测试输出、身份配置和构建产物保存在 Git 忽略的
`.local/`、`target/`、`dist/` 中，不作为公开仓库内容。

使用 `python3 -m http.server 8765 --bind 127.0.0.1 --directory .agents` 启动
设计原型，访问 `http://127.0.0.1:8765/design/`。原型中的操作仅修改示例数据。
模块划分见 [架构文档](docs/ARCHITECTURE.md)。

## 许可证

项目原创代码、文档和项目素材采用 [Apache-2.0](LICENSE)。第三方依赖与图标保留
原有许可，见 [第三方声明](THIRD_PARTY_NOTICES.md) 和
[依赖清单](docs/DEPENDENCIES.md)。

贡献流程见 [CONTRIBUTING.md](CONTRIBUTING.md)，安全问题请按
[SECURITY.md](SECURITY.md) 私下报告。ContainerHub 是独立项目，与 Apple、Docker
没有从属关系。
