# 增量 11：创建后启动与部分结果恢复

2026-10-22。桌面、Node 和直连/中继 Agent 已接入可选“创建后立即启动”。默认仍创建停止的容器。本轮只使用临时夹具、FakeProvider 和 CLI 模拟器，没有操作用户运行时或容器。

## 行为与公共模块

`CreateOptions.start_after_create` 缺省 false，false 时不增加序列化字段，保持原有命令与历史格式。`create_and_start` 是独立能力，旧快照缺省 false；Apple 适配同时要求基础创建和生命周期契约。诊断共 19 项。远程快照/诊断还要求当前环境的 ManageContainers 和 Lifecycle 两项授权。

公共 `core/workflow` 执行创建 → 保存完整容器身份 → 启动 → 重新查询运行状态。provider 只执行单步操作，直接传立即启动选项会被拒绝；桌面和 Node 使用 OperationService，Agent 使用相同公共流程。核心流程、队列检查点、Agent 检查点、桌面启动选项均放在独立模块。

创建确认后，`ExecutionControl` 保留不含秘密值的 Container 结果，并等待 `CreationCheckpoint`：本机队列写原子 JSON 后确认，Agent 写 SQLite 后再复查提交设备的两项授权。检查点写失败时不继续启动，失败的写入不会被缓存为已成功确认。创建期间设备被吊销时保留创建结果，但不执行启动。

启动失败仍返回 Failed；结果不明确时返回 UnknownOutcome。这两种状态下 `Operation.result` 可以保留 Container，表示创建已确认，不表示启动成功。任务中心显示“已创建 · 启动未完成/待核对”；重新查询确认同一 incarnation 处于停止状态后，“启动已创建容器”只提交一个新的 Start 任务，不重新 Create，不需要再次提供秘密值。同名替代对象不显示这个入口，provider 也会拒绝旧身份。

创建命令收到超时但尚未确认完整身份时，无法形成检查点，继续保留未知结果，不根据名称猜测创建成功。没有自动删除或回滚已创建的容器。

## 中断与恢复

- 本机队列中断后，使用保存的创建身份查询：同一对象运行中可确认完成，停止状态保留已创建结果并提供单独启动；缺失、替换或无法查询时保持未知。不重放创建或自动补发启动。
- 控制端拥有远程 receipt 时，完整命令交给 Agent 执行；控制端只查询原 ID，收到创建结果后同步到本机检查点。直连与中继共用同一任务关联。
- Agent 自身重启时，未终结任务继续标记 UnknownOutcome，保留已保存的创建结果，不自动补执行。Agent 终态是历史证据，不用之后的资源状态覆盖原任务结果。
- 断线后的结果、已知创建身份和原请求关联不会因核对失败而被清空。同 ID 重交只查询原记录；秘密值仍只能提交一次，值不进入两层任务日志。

Apple CLI 按名称寻址，执行前后检查 incarnation 能拒绝已存在的替代对象，但不能消除外部 CLI 在检查与实际调用之间替换同名对象的竞态。

## 验证

- fmt、workspace/all-targets/all-features clippy（warnings denied）和 67 项 Rust 测试：desktop 8、core 13、Apple 19、management 8、remote 19。
- 本机真实队列 worker 中断：分别在已创建、启动请求等待中与运行状态已改变但完成结果未返回时 abort；重开队列只观察状态，创建和启动计数均不增加。检查点写入失败不启动；重复检查点不会把失败当 ACK；同名重建后拒绝启动旧 key。
- 直连/中继控制端中断：带秘密环境与启动参数的创建仅执行一次；Agent 在启动前已经保存创建结果；恢复最终确认运行。权限测试覆盖缺少 Lifecycle 时创建前拒绝、创建期间吊销后禁止启动、通过当前只读设备查询保留的部分结果。Agent 重载使用持久化阶段夹具，保留身份与 UnknownOutcome。
- Node → bridge → Apple CLI 模拟器：默认停止、创建并启动、启动失败保留结果、重启与原 ID 去重、重新提交 Start、同名替换拒绝、秘密值不进入 argv/任务结果、缺少启动能力仍允许普通创建。
- Node 远程旅程验证环境身份映射与创建后运行；WebCrypto/WASM 的只读设备能力过滤与拒绝写入继续通过。既有 SDK 错误进程、诊断、秘密输入、启动参数、生命周期、镜像和日志回归通过。
- GPUI 实际组件六组结构/交互检查：亮/暗色 × 760×560、1040×700、1440×900。覆盖立即启动选择、部分结果重试只发送 Start、无需再次输入秘密、同名替换隐藏重试以及窄窗口按钮边界。

本轮再次尝试电脑操作服务，仍返回 `Sky Computer Use service startup request failed`。上述 UI 检查不含原生像素、IME、系统剪贴板或 VoiceOver。网络测试使用 loopback，不是公网/NAT 双主机验收；CLI 模拟器不是实际 Apple 容器。没有重复探测或恢复用户的 Apple 服务。

开发包位于 `dist/ContainerHub.app`、`dist/bin`、`dist/node` 和 `dist/browser-protocol`。普通 Agent/bridge 拒绝 development 身份；app 的 ad hoc 签名、bundle 身份和 19 个 Mach-O 文件内节已与编译产物核对；打包的 Node SDK/bridge 完成带启动配置和秘密变量的创建后启动、检查、停止及删除流程，SDK 源码/类型与工作区一致。未安装到 /Applications、初始化 Git、提交或发布。

验证日志：`.local/validation-increment-11.txt`、`.local/packaging-increment-11.txt`、`.local/native-ui-increment-11.txt`。

M1 仍缺服务启动入口、任务归档维护以及真实 Apple/原生视觉验收。Docker/Kubernetes provider、正式应用证明签发、真实公网/NAT、设备加入 UI、完整 Web、加密同步和恢复/根轮换仍待实现。
