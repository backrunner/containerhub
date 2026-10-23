# 增量 8：跨层任务关联与恢复

2026-10-22。本轮解决桌面/Node 本机任务与 Agent 请求只在进程内关联的问题。未处理系统授权，也未操作用户容器。

## 发送前持久化

core 新增 `ExecutionReceipt`，只含 Agent 公钥 ID、所有者公钥 ID、原请求 ID 和原期限，不含凭据或私钥。公共队列在任务开始时生成关联，将它与原命令一起写入 `operations.json`，落盘成功后才发送 Submit。保存失败不会发送命令。

`ManagementClient` 增加 prepare/execute_prepared/resume_execution 边界；Apple 等本机 provider 默认没有远程关联。RemoteClient 的变更与恢复拆入 `client/operations`，公共队列的执行及控制拆入 `queue/execution`、`queue/control`。桌面、Node bridge 共用同一套持久化和恢复逻辑。

## 重启与人工核对

- 重启发现带关联的 Running、Reconciling 或 UnknownOutcome 时，最多自动排入 32 项核对；超出部分保留未知，可手动继续。远程查询在服务 Ready 后运行，不因 Agent 离线阻止 bridge 启动。
- 核对与新任务共享最多 4 项并发及同资源串行。查询原 Agent 请求，检查 Agent、所有者、request ID、命令和原期限均匹配；直连中断后可通过同一绑定的中继查询。
- Agent 的明确结果写回原本机任务 ID，保留返回的创建结果、进度及取消状态。请求过期不妨碍读取已存在的历史记录。
- 查询失败、权限失效、绑定改变、记录缺失或返回记录不匹配时保持 UnknownOutcome。不会自动重新 Submit，也不会通过“容器已经不存在”推断创建或删除成功。
- Agent 自身中途重启后仍将未完成记录保留为 UnknownOutcome；本轮不猜测守护服务是否完成了写入。旧本机日志没有 request ID 时无法补造关联，仍仅能对相同 incarnation 的 Start/Stop 做有限状态核对。
- 本机未执行的 Queued 仍在重启时标为失败，不自动重放。损坏日志拒绝写入、去重容量与原有取消持久化顺序不变。

恢复队列中的远程写操作可能已在 Agent 执行，因此取消它不能标成“尚未执行”。持久化取消意图后只对原 request ID 发送幂等 Cancel；以前已保存但未确认的取消也可继续交付，Submit 始终不重送。

## 桌面与 Node

任务中心显示 Agent 请求 ID，“核对原任务”会真正更新原任务状态，不再只刷新容器列表。Node 新增 `reconcile(operationId)`；可随后 `wait` 查询。对活动任务或已有明确终态的任务返回原记录；仍未知时可以在网络/权限恢复后再次核对。

bridge 的 v1 协议新增 `reconcile`，Agent 复用现有 operation 查询，不增加任意命令执行入口。查询每次验证当前 Read 权限、环境范围和所有者；没有记录返回 NotFound，客户端将其解释为无法确认结果。

## 验证

- fmt、workspace/all-targets/all-features clippy 通过；57 项 Rust 测试通过：desktop 8、core 9、Apple 19、management 5、remote 16。
- 加密 loopback WebSocket 测试实际中断本机队列工作任务，在 Agent 仍执行时重建公共服务，验证原关联落盘、原 ID 去重、恢复后仅执行一次；包括直连恢复与从直连切到出站中继。
- 覆盖保存关联失败不提交、核对落盘失败不排队、恢复排队期间取消不误报、最多 32 项启动核对、缺失记录不重放、过期历史仍可查询、错误绑定/返回记录拒绝、当前 Read 权限检查和 Agent 重载后的未知结果保留。
- Node → bridge → Agent 的隔离历史夹具模拟丢失本机最终落盘，重启 bridge 后取回原创建结果及 receipt；目标容器已在后续流程删除，恢复不会重新创建。该测试是历史夹具，不冒充进程在精确指令点被强杀。
- 6 组 GPUI 结构/交互检查通过：浅/深色 × 760×560、1040×700、1440×900；覆盖核对按钮只发原 ID、核对中状态和保留原关联。既有管理/日志、Node 异常进程、CLI 模拟器、WebCrypto/WASM/直连/中继/在线吊销回归通过。
- 开发 app、普通 Agent/bridge、Node 包及浏览器协议重建。普通构建拒绝 development 身份；app ad hoc 签名及 19 个 Mach-O 文件内节与构建产物匹配；打包 SDK 验证 reconcile 和既有管理 API。

记录：`.local/validation-increment-8.txt`、`.local/packaging-increment-8.txt`。保留修正前失败与最终通过输出。未安装到 /Applications、初始化 Git、提交或发布。

## 仍待完成

真实 Apple 运行时、原生像素/IME/系统剪贴板/辅助功能、公网/NAT 双主机故障旅程未验收。本轮没有重复探测或启停用户运行时；最近一次服务/原生操作探测见增量 7。

M1 仍缺秘密注入等完整创建能力、服务启动入口和任务归档维护；旧记录和 Agent 崩溃后无法确认的执行仍需人工核查资源。Docker/Kubernetes、正式应用证明签发、设备加入 UI、完整 Web、加密同步和恢复/根轮换尚未实现。
