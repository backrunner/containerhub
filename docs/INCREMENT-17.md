# 增量 17：本机运行时服务启动

2026-10-23。补齐服务离线时的启动入口、公共执行路径和结果核对，桌面、Node 与设计原型同步。M1 仍待任务归档维护、真实运行时及剩余系统交互验收。

## 使用流程

资源页离线提示提供“检查环境”。环境管理在 CLI 信息下显示紧凑服务状态区；本机离线且具备启动能力时提供“启动服务”。确认弹窗说明仅启动后台服务、检查连接，以及首次内核安装和 macOS 权限需手动完成。

运行中禁止重复提交；未知结果改为“核对结果”，不显示直接重试启动。完成后刷新资源和诊断，操作记录进入既有任务中心。确认期间环境、CLI 路径、版本或可用状态改变时拒绝提交。没有容器快照也能启动；权限不足、CLI 未安装、状态格式不明或未验证版本不开放启动入口。

弹窗宽 540 pt、内边距 24 pt，按钮高 32 pt、间距 8 pt、右对齐。服务状态区采用既有边框、字号与颜色，保持宽窗口两列、窄窗口纵向排列。原型更新至 v5.5，支持 `?page=runtime&scenario=offline` 与 `?page=runtime&service=unknown`；原型仅模拟示例状态，不运行本机命令。

## 公共模块和执行边界

- core 新增 `Command::StartRuntime { environment }` 和 `RuntimeFeature::StartRuntime`；诊断共 21 项。启动能力独立于资源快照，同环境启动使用独立运行时锁。
- Apple 的 `system` 模块重新检查版本和帮助契约，已运行时直接确认；离线时使用固定参数 `system start --disable-kernel-install --timeout 30`，进程上限 40 秒、无交互 stdin。不会自动下载内核、提升权限或执行容器生命周期命令。
- 命令结束后再次查询服务状态。仅确认服务运行才记为成功，不以 CLI 退出码代替后置状态；超时或退出成功但服务不可用保留 UnknownOutcome。权限错误按失败报告，原始 CLI 输出不进入任务文案。
- management 的持久化、同 ID 去重、任务队列保持共用。启动期间重启或人工核对只读取诊断，不再次调用 start；离线恢复不会依赖 inventory 查询成功。
- 桌面新增独立 `runtime_service` 展示/确认模块；Node 通过既有 `submit`/`execute`/`reconcile` 访问同一执行逻辑，类型和 SDK 文档同步。
- **服务启动首版仅限本机**。远程 client 在发送前拒绝，Agent 再次独立拒绝，并在能力报告中标为不可用；现有 Lifecycle/ManageContainers 授权没有扩大到主机服务管理。

CLI 依据：本机已安装 1.6.0 的只读 `system start --help` 和 [Apple 1.6.0 command reference](https://github.com/apple/container/blob/1.6.0/docs/command-reference.md)。帮助中的组合声明 `--enable-kernel-install/--disable-kernel-install` 已纳入能力解析；未验证版本仍禁止写入。

## 验证与证据

- 87 项 workspace/all-features Rust 测试通过；rustfmt 和 workspace/all-targets/all-features Clippy `-D warnings` 通过。新增启动并发去重、退出码与服务状态分歧、缺能力/未知版本/错误环境/非法响应拒绝、权限拒绝、超时不重试，以及远程 client/Agent 双向拒绝。
- Node 真实 stdio bridge → 隔离 Apple CLI 模拟器：无快照时启动、成功后诊断、重复请求、未知结果跨重启保留、显式核对、在线/离线中断恢复均通过，核对期间启动次数不增长。
- Node 管理、异常 EOF/关闭、诊断、创建后启动旅程通过。Node 远程管理及 WebCrypto/WASM/WebSocket 直连/中继、授权、吊销回归通过；远程 21 项诊断中启动服务不可用，Node 请求被拒绝。WASM 加密模块未改动，使用此前生成的本地测试绑定；这是 Node 互操作证据，不是完整浏览器远程产品验收。
- 六组 GPUI 结构、交互和 Metal 离屏渲染：亮/暗 × 760×560、1040×700、1440×900。覆盖离线无快照提交、取消不写入、CLI 变化拒绝、重复点击禁用、UnknownOutcome 只核对、完成后诊断刷新和本机范围限制。新服务页/弹窗 12 张，累计 96 张 PNG，位于 `.local/ui-validation/`。
- Edge 原型模拟启动后显示服务可连接，未知结果保留并提示未重复启动；六组窗口/主题没有横向溢出，弹窗和确认按钮均完整可见。截图位于 `.local/visual-review/design-runtime-*.png`。
- 开发 app 与 Node 包已重新生成。独立 `ContainerHub Preview.app` 保持显式 `--demo`，替换程序并验证 ad hoc 签名后重新启动；实际原生环境管理页显示新服务状态区和 21 项能力。截图为 `.local/visual-review/native-runtime-service.png`。离线启动交互由 GPUI 隔离测试和 Node CLI 模拟器验证，没有将 Demo 在线状态当作真实服务启动证据。

本轮没有连接、启动、停止或修改用户真实 Apple container，也没有处理用户暂缓的系统授权。没有安装到 /Applications、发布、推送或公证。Docker/Kubernetes provider、远程设备加入界面、正式应用证明、完整 Web 与云端同步仍在后续范围。
