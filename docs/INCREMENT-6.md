# 增量 6：拉取控制、高级创建与容器检查

2026-10-22。桌面、Node 与远程 Agent 共用本轮契约；没有改动用户运行时授权或用户容器。

## 拉取进度与取消

- `Operation.progress` 为 `waiting/fetching/unpacking/verifying`，只展示已观察到的阶段。Apple 1.6.0 使用 `image pull --progress plain`；适配器只识别固定阶段，原始输出不进入 UI、RPC 或日志。不估算百分比、下载字节或剩余时间。
- `ExecutionControl` 位于 core，不依赖 Tokio、GPUI 或传输。management 队列最多每 200 ms 发布一次发生变化的进度，最终进度随结果保存；不为每个输出块重写整个日志。
- 桌面任务中心提供取消；Node 使用 `cancel(operationId)`，远程使用同一 Agent request ID 的 `cancel`。取消意图先持久化再通知执行器；失败不会发送取消。已结束的拉取返回原结果，重复取消幂等。
- 尚未执行的任务记为 `Cancelled`。运行中的 Apple 拉取结束 CLI 子进程并回收进程，随后查询镜像引用，结果仍为 `UnknownOutcome`：守护服务可能继续下载，已有引用也不能证明本次拉取完成。不会删除下载内容或报告已回滚。若成功结果已确认，保留 `Succeeded`。
- Agent 按所有者及目标环境查询原任务，并重新验证 `ManageImages` 权限。只读设备不能取消；Queued→Running、取消与最终落盘之间序列化，重启不重放。
- Node `wait/execute` 支持 `onProgress(operation)`；`AbortSignal` 仍只终止等待，取消运行时必须显式调用 `cancel`。

Apple 行为依据：[ImagePull](https://github.com/apple/container/blob/1.6.0/Sources/ContainerCommands/Image/ImagePull.swift)、[ProgressBar](https://github.com/apple/container/blob/1.6.0/Sources/TerminalProgress/ProgressBar.swift) 与本机 1.6.0 `--help`。创建 mount 指令还按 [Parser](https://github.com/apple/container/blob/1.6.0/Sources/Services/ContainerAPIService/Client/Parser.swift) 核对。

## 高级创建

`CreateContainer.options` 向后兼容，缺失时使用默认值：

| 字段 | 范围 |
| --- | --- |
| `networks` | 最多 8 个去重网络名称，执行前确认存在 |
| `volumes` | 最多 16 个已有命名卷，包含 name/destination/read_only；拒绝宿主路径、根挂载和重复目标 |
| `user` | 用户或 UID，可附组/GID |
| `working_directory` | 容器绝对路径，拒绝控制字符、参数分隔符及路径回退 |
| `read_only` | 只读根文件系统 |

桌面高级区域可折叠，表单可滚动，提交按钮保留在底部；端口支持多组 loopback TCP/UDP。公共端口契约仍支持最多 32 组显式 IP 映射。命令参数由 Rust 类型构造，不经过宿主 shell；创建后保持停止。卷/网络在外部 CLI 并发修改下仍有检查与使用之间的竞态。

本轮未加入环境变量值、自定义启动参数、宿主目录挂载、自动删除或新增卷/网络。环境值与命令参数可能携带凭据，后续需独立的秘密注入与存储设计，不能直接放进持久化 `Command`。

## 脱敏容器检查

`inspect_container(ResourceKey)` 返回 `ContainerDetails`。Apple 在调用公共 `inspect` 前后检查 incarnation，并核对返回身份。适配层采用允许字段列表，环境变量与标签仅返回名称，启动参数只返回数量；不提供原始 inspect JSON。

桌面增加「检查」页、刷新和复制 JSON；关闭或更换选择后拒绝过期结果。Node 为 `inspectContainer(key)`；远程读取需 Read 权限，环境映射保持原资源键。基础配置、网络、挂载和资源身份也包含在 DTO 内。

## 模块与验证

新增 core 的 `execution/create_options/inspect` 模块，Apple 的 `inspect` 适配模块，桌面的高级表单与检查视图；Agent 操作受理/执行/取消拆到 `server/operations.rs`。没有在 Node 或 UI 重写容器操作。

- 42 项 Rust 测试：desktop 7、core 8、Apple 13、management 2、remote 12；fmt、all-targets/all-features clippy 通过。
- CLI 模拟器验证进度、子进程取消、执行前取消、公共参数、高级创建、缺失卷拒绝、脱敏与身份。公共队列验证排队取消、运行中结果未知、取消落盘失败不通知执行器、幂等取消、恢复不重放。真实 loopback WebSocket 直连/中继验证远程进度、取消、权限和去重。
- 6 组 GPUI 结构/交互检查：浅/深色 × 760×560、1040×700、1440×900，包括展开高级选项、滚动、TCP/UDP、检查复制/过期响应与任务取消按钮。不是原生像素验收。
- Node 本机/远程、异常子进程、WebCrypto/WASM、普通构建的 development 身份拒绝通过。开发 app、Agent/bridge、Node 包与浏览器协议已重建；app 签名和 19 个文件内 Mach-O 节匹配编译产物，打包 Node 实际执行新创建/检查 API。
- 记录：`.local/validation-increment-6.txt`、`.local/packaging-increment-6.txt`、`.local/runtime-increment-6.txt`。回归记录保留修正前的失败和最终通过输出。

本机只读服务探测仍返回 `launchctl managername` status 153。电脑操作服务本轮启动失败，无法取得原生截图；真实 Apple 运行时、原生视觉、系统保存窗口与公网/NAT 验收仍待完成。没有安装到 /Applications、初始化 Git、提交或发布。

M1 仍缺秘密注入等完整创建能力、日志选区复制、完善的能力发现与跨层恢复；Docker/Kubernetes provider、正式应用凭据签发、完整 Web 和加密云同步也未完成。
