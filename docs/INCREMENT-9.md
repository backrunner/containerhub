# 增量 9：一次性秘密环境变量

2026-10-22。桌面、Node SDK、公共任务队列、直连/中继 Agent 与 Apple provider 接入同一条秘密输入通道。只使用隔离模拟器和 FakeProvider 验证，没有启停用户运行时或操作用户容器。

## 输入和持久化边界

- `CreateOptions.secret_environment` 只声明变量名；实际值由独立的 `SecretEnvironment` 进入 `OperationRequest` 和 `ExecutionControl`，不会进入 `Command`、`Operation`、receipt、普通返回 DTO 或 Agent SQLite 请求/结果。队列与执行有界，完成、拒绝或丢弃输入后释放值。
- 名称使用 `[A-Za-z_][A-Za-z0-9_]*`，最多 128 字节、32 项；每值最多 16 KiB，编码为 env-file 后总计最多 64 KiB。支持空值、空格、等号、引号、中文等 UTF-8 字符；拒绝 NUL 和 Foundation 会分行的所有换行字符。声明与实际输入必须完全对应，不会隐式继承主机变量。
- 调用方提交含值的同一 operation/request ID 一律冲突，即使公共命令相同，也不将新值默认为已经应用。只查询原 ID 可以取得旧结果；再次创建必须明确使用新 ID。并发受理仍由现有原子日志/SQLite 去重控制。
- 本机排队输入不落盘，重启后标记失败并提示重新填写；已发送的远程任务继续用增量 8 的 receipt 查询原 Agent 记录，不重传秘密或重放创建。Agent 自身中断的未完成任务继续保留 UnknownOutcome。
- 远程新增 `submit_with_secrets { request, secrets }`，仅在已认证的 Noise 通道中发送。Agent 在受理和实际执行时核对 ManageContainers 与环境权限，读取时继续核对 Read。没有增加 shell 或任意文件写入入口。

`SecretEnvironment` 的 Debug 只显示名称，Rust 所拥有的值、管道输入、桥接帧和 Noise 明文缓冲区在释放时清零；Node 写入缓冲区在回调中清零。这不是清除所有内存副本的承诺：GPUI 编辑器、JavaScript 字符串、序列化器、内核管道及运行时可能持有副本。没有新增持久化秘密库、Keychain 秘密编辑器或跨设备秘密同步。日志是容器自己产生的内容，应用不能保证容器不会主动打印秘密。

## Apple 与能力发现

新增独立 `secret_environment` 诊断项和 `create_secrets` 能力，当前共 17 项。Apple 要求适配版本 1.6.0、基础创建契约及 `--env-file`；不因网络/挂载等高级选项缺失而禁止单独秘密注入。

创建使用 `--env-file /dev/stdin`，通过子进程 stdin 管道传递 `KEY=VALUE`，不创建秘密临时文件、不把值放入 argv 或主机进程环境。输入写入与 stdout/stderr 读取并行，覆盖在现有创建超时内；输入中断保留结果未知，运行时原始错误不会直接返回。创建后仍保持停止。

依据 [Apple 1.6.0 Parser.envFile](https://github.com/apple/container/blob/1.6.0/Sources/Services/ContainerAPIService/Client/Parser.swift#L174) 的管道读取及分行契约实现。源码核对和 CLI 模拟器不等于真实 Apple 创建验收。值最终交给容器运行时，容器、运行时配置和有权限的主机用户仍能读取。

## 桌面与 Node

创建表单可添加/移除多项秘密变量，值默认遮罩，窄窗口沿用可滚动表单和固定确认栏。没有能力时禁用添加入口；失败任务不会提供无值重试。字段提示说明值不进入任务记录、运行时仍可读取。UI 模块拆入 `forms/secrets`，共同校验在 `core/secrets`，SDK 不复制执行逻辑。

Node 使用 `submit(command, { operationId, secrets })` 或 `execute(command, { secrets })`；命令只保存 `options.secret_environment` 名称列表。示例见 [SDK README](../packages/node/README.md)。普通 `submit` 和旧无秘密命令兼容；结果、进度回调、inspect 与历史仍仅含允许字段。

## 验证与产物

- fmt 与全 workspace/all-targets/all-features clippy（warnings denied）通过；60 项 Rust 测试通过：desktop 8、core 11、Apple 19、management 5、remote 17。
- 新增核心边界、大小限制、特殊字符与 Debug 脱敏测试。直连及出站中继实际中断控制端队列工作任务，确认 Agent 已收到原值，恢复后原 ID 仅执行一次；并发重复秘密提交被拒绝，SQLite 文件和返回记录没有 canary 值。
- Node → Rust bridge → 公共队列 → Apple CLI 模拟器校验 stdin 字节摘要、名称映射、空值及特殊字符保真，argv/历史/DTO/错误无 canary；缺失/多余输入、换行/超长输入被拒绝；排队输入丢失和缺少 `--env-file` 均不执行创建。
- Node 远程创建带秘密，重新打开本机丢失最终结果的历史夹具后按原关联恢复；WebCrypto/WASM 只读设备提交秘密被拒绝。既有管理、进度/取消、错误进程、直连/中继及在线吊销回归通过。
- 浅/深色 × 760×560、1040×700、1440×900 的 6 组 GPUI 结构/交互检查通过；覆盖变量输入、遮罩字段不暴露辅助功能值、滚动和确认栏、提交载荷与历史分离。这不是原生像素、系统 IME/剪贴板或 VoiceOver 验收。本轮电脑操作服务再次启动失败，未能进行原生窗口检查。

本机只读核对 CLI 1.6.0 的 `create --help` 声明了 `--env-file`，并独立验证 macOS Foundation 从 `/dev/stdin` 管道原样读取字节；没有连接服务或创建容器。记录：`.local/apple-secret-contract-increment-9.txt`。

普通 Agent/bridge 拒绝 development 身份；开发 app 的 ad hoc 签名与 19 个 Mach-O 文件内节内容匹配构建产物，打包后的 Node SDK/bridge 完成带秘密创建、脱敏检查及既有管理流程。

验证日志：`.local/validation-increment-9.txt`，保留修正前 Node 能力断言失败及修正后的通过结果。打包日志：`.local/packaging-increment-9.txt`。开发产物为 `dist/ContainerHub.app`、`dist/bin`、`dist/node`、`dist/browser-protocol`；未安装到 /Applications、初始化 Git、提交或发布。

M1 尚缺自定义启动命令等完整创建能力、服务启动入口、任务归档维护及真实运行时/原生视觉验收。Docker/Kubernetes、正式应用证明签发、公网/NAT 双主机、设备加入 UI、完整 Web、加密同步及恢复/根轮换仍未完成。
