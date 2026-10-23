# 增量 16：端口重建与操作布局

2026-10-23。将详情中的端口映射改为编辑入口，公共命令、Apple 适配器、Agent、Node SDK 和设计原型同步。

## 交互和视觉

- 点击 `3000 → 3000` 打开 580 pt 编辑弹窗；独立复制按钮复制完整映射。多条映射显示数量，避免详情栏溢出。
- 可添加、删除映射，编辑主机 IP、主机端口、容器端口、TCP/UDP；最多 32 条。校验有效 IP、1–65535 整数端口、通配地址冲突和重复映射。长列表在弹窗内滚动。
- 确认按钮为红色“重建”。弹窗明确提示删除原容器、可写层丢失、挂载卷和目录保留、恢复原运行状态，以及失败不回滚。不会留备份容器。
- 行操作组固定 136 pt：启停 64×28、重启和更多各 28×28，边框及 8 pt 间距统一。停止维持红色危险样式。
- 内存显示为 `48 MiB / 512 MiB`，单位及上限使用同一小字号基线。取消原来的第二行“上限”。
- 原型更新至 v5.3；对照页新增“端口重建”和“端口与内存上限”。

## 执行与恢复

`Command::RebuildPorts` 携带完整资源身份、编辑时运行状态、旧端口与目标端口。校验与能力声明在 core；任务持久化、替换身份检查点和恢复逻辑在公共 management/workflow；GPUI 只负责表单。Apple 配置转换拆为 plan、process、attachments，表单字段也独立成模块。

Apple 1.6.0 适配器在任何停止/删除之前检查 CLI 能力、容器身份和配置、镜像摘要及平台默认值、网络/卷引用、端口冲突。仅翻译能够完整保留的公开 inspect 字段；不认识的字段或无法重现的配置直接拒绝。使用摘要固定原镜像，环境变量值经 stdin 注入，不放进命令、任务日志或 UI。

顺序为停止原容器（原本运行时）→确认身份→删除原容器→确认同名资源不存在→按原镜像、配置和新端口创建→核对新身份及配置→持久化检查点→恢复运行/停止状态。没有快照镜像或备份容器。CLI 删除不使用 force。

创建失败或结果不明不自动重复删除/重建。已验证的新容器若启动失败，可单独重试启动该身份；替换身份、端口或运行状态不匹配时不标为成功。Agent 要求 ManageContainers 和 Lifecycle 两项权限，沿用原任务查询、去重及安全通道。Node 类型和任务进度已同步。

## 限制

- 原容器可写层会删除，失败不能恢复。进程中断且尚未形成新容器检查点时，只保留任务结果待核对，不在磁盘保存含秘密值的原始配置供自动重放。
- Apple 公开 inspect 不提供 autoRemove；新容器不启用退出自动删除，真实运行时弹窗说明此差异。
- 不能完整复现的配置（如非标准挂载、未连接网络、附加组、发布套接字、sysctls、自定义存储配额等）会在停止前拒绝。此轮支持 Apple 1.6.0 的已验证公开 CLI 契约，未扩展 Docker/Kubernetes。
- 其他本机进程的端口占用和外部 CLI 竞争仍可能导致创建/启动失败；不能把应用内检查当成跨进程事务。

参考：[Apple command reference](https://github.com/apple/container/blob/main/docs/command-reference.md)、[1.6.0 ContainerConfiguration](https://github.com/apple/container/blob/1.6.0/Sources/ContainerResource/Container/ContainerConfiguration.swift)。

## 验证

- Rustfmt 与 Clippy workspace/all-targets/all-features `-D warnings` 通过。
- 82 项 workspace Rust 测试通过；新增停止/运行状态重建、原身份消失、配置拒绝、删除后创建失败不重放、启动失败仅重试启动、端口冲突/IPv6、恢复目标状态和远程权限覆盖。
- Node SDK Demo 旅程通过：真实 stdio bridge 提交重建，身份改变、仅保留一个同名容器、配置与运行状态保留、旧请求不能删除替代容器；异常 EOF/关闭检查通过。
- 六组 GPUI/Metal 验证：亮/暗 × 760×560、1040×700、1440×900。实际表单输入与提交、无效端口拒绝、陈旧身份拒绝、按钮尺寸、行间距和多行滚动检查。每组新增概览和端口弹窗，共 84 张 PNG。
- Edge 原型验证无效端口和成功模拟替换；六组窗口/主题无横向溢出，弹窗完整可见。截图和尺寸记录在 `.local/visual-review/`。

- 独立原生 Demo 预览已更新并启动，实际将 web-api 从 3000 → 3000 重建为 3001 → 3000，列表仍为 8 个容器、仅一条 web-api、保持运行中。保存 `native-ports-modal.png` 和 `native-ports-rebuilt.png`；开发包和预览包 ad hoc 签名验证通过。

本轮仅使用 Demo/FakeProvider、隔离 Apple CLI fixture 和既有脱敏 1.6.0 inspect 样本验证。没有连接或修改真实本机容器；系统授权仍由用户后续处理。原生窗口验收与离屏、浏览器证据分别记录；没有安装到 /Applications 或发布。
