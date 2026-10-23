# 调研依据与验证边界

核查日期：2026-10-21（Asia/Singapore）。仅进行公开资料和本机只读查询。

## 1. 本机快照

| 项目 | 观察结果 |
| --- | --- |
| 工作目录 | 项目根目录，调研时为空且尚非 Git 仓库 |
| CLI | `/opt/homebrew/bin/container`，指向 Homebrew `container/1.2.2_1/bin/container` |
| CLI 版本 | `container CLI version 1.2.2` |
| 服务 | `container system status` 返回 running，apiserver 1.2.2 |
| 子命令 | 生命周期、copy/export、logs/stats、image、registry、build/builder、network、volume、machine、system |
| 插件 | `k8s`，help 明确标为 EXPERIMENTAL |
| 结构化读取 | list、stats、system status/df 支持 JSON；inspect 为结构化详情入口 |
| Registry 登录 | help 提供 `--password-stdin` |

执行过 `--version`、根 `--help` 和上述关键子命令的 `--help`，以及 `system status`。未列举用户容器内容，未读取私有镜像、凭据、业务日志或 inspect 中的敏感环境变量；未运行 create/start/stop/delete/prune/build/login。

该快照只证明命令存在和服务可查询，不证明生命周期、PTY、资源指标或任何产品功能已通过运行验证。

## 2. 官方来源

| 来源 | 支撑的判断 |
| --- | --- |
| [Apple container README](https://github.com/apple/container) | Apple silicon/macOS 26、OCI 镜像、CLI/XPC 兼容范围和实验功能边界 |
| [Apple 1.6.0 发布说明](https://github.com/apple/container/releases/tag/1.6.0) | 2026-10-20 发布；commit；build/builder 插件化；k8s 多 worker；含安全修复 |
| [1.6.0 命令参考](https://github.com/apple/container/blob/1.6.0/docs/command-reference.md) | 稳定功能入口、JSON 支持、commit/clean、machine、实验性 k8s |
| [1.6.0 技术概览](https://github.com/apple/container/blob/1.6.0/docs/technical-overview.md) | 每容器轻量 VM、CLI 客户端与 XPC 服务、launchd/Keychain 集成 |
| [GPUI 官方](https://gpui.rs/) | Rust GPU UI 与原生实体/渲染模型；具体依赖兼容需工程验证 |
| [旧 GPUI Component 入口](https://longbridge.github.io/gpui-component/) | 文档已声明迁至 GPUI Kit |
| [GPUI Kit](https://gpui-kit.com/) | 当日文档版本 0.8.0；组件、虚拟表格、主题、辅助功能与测试能力；本项目尚未编译验证 |
| [Portainer Docker 管理](https://docs.portainer.io/user/docker) | 容器、镜像、网络、卷、Stacks、事件、Host 等对标类别 |
| [Portainer 容器管理](https://docs.portainer.io/user/docker/containers) | 创建、检查、编辑、日志、统计和控制台工作流 |
| [Portainer Kubernetes 管理](https://docs.portainer.io/user/kubernetes) | Namespace、Application、网络、配置、卷和集群 |
| [Portainer Roles](https://docs.portainer.io/admin/user/roles) | Business Edition 的细粒度权限及 Docker/Kubernetes 授权差异 |
| [Docker Engine API](https://docs.docker.com/reference/api/engine/) | Docker 接入应处理 API 版本和兼容范围 |
| [Kubernetes API concepts](https://kubernetes.io/docs/reference/using-api/api-concepts/) | Kubernetes 独立资源语义及 list/watch 模型 |

## 3. 适配决策：观察与推断分开

- **已观察**：本机 1.2.2 根命令无通用 restart、pause、events、compose；上游 1.6.0 命令参考未列这些入口。**设计推断**：首版重启采用 stop/start；状态同步采用轮询。未来发现新的公开能力时替换实现。
- **已观察**：CLI 有 JSON 选项。**尚待验证**：实际 JSON schema、stats 流分帧、错误输出、取消行为、终端 resize、宿主权限和版本差异。M0 必须补齐 fixtures 与真实试验。
- **已观察**：上游有公开 Swift 客户端与 XPC 架构。**设计选择**：首版 Rust 调 CLI，暂不桥接 XPC。这是维护成本和稳定性取舍，不是「Apple 没有 API」。
- **已观察**：1.6.0 构建入口插件化。**设计选择**：能力表同时检查主 CLI 和插件；版本字符串不能替代功能探测。
- **已观察**：已有实验性本地 k8s。**设计选择**：在后续 Kubernetes 阶段分离集群生命周期与集群工作负载管理；当前 UI 不展示为已支持。
- **未验证**：Mac App Store 分发适配、跨平台桌面、远程 WAN 连通、签名安装、完整 Portainer 对标、任何性能数字。计划中的数字均是验收目标。

## 4. 依赖与安全基线

在 M0 记录 GPUI/GPUI Kit 与 Rust toolchain 的可编译组合、许可证清单和锁文件。不要把历史项目中的旧 GPUI Component 版本直接复制到新项目，也不要把当前文档中的最新版本当作已测试组合。

Apple 1.6.0 发布说明列出了镜像 digest 验证及构建上下文相关修复。产品应提供版本风险和更新入口；升级需要维护窗口、兼容检查和用户发起。本次调研没有修改本机 1.2.2，也未做漏洞复现。
