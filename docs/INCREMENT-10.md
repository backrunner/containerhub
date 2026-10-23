# 增量 10：容器启动命令

2026-10-22。桌面、Node SDK、直连/中继 Agent 接入共享的启动配置；创建仍保持停止，由用户独立启动。没有启停用户运行时或操作用户容器。

## 共享契约与执行

`CreateOptions.startup` 是可选的 `StartupCommand`，包含可选 `entrypoint` 与有序 `arguments` 数组。未设置时保留镜像默认配置；只提供参数时保留镜像 ENTRYPOINT 并替换 CMD；提供启动程序时替换 ENTRYPOINT，同时丢弃镜像 CMD，即使参数为空。空字符串启动程序与空的覆盖对象被拒绝；参数数组中的空字符串则保留。当前没有提供“清空镜像 ENTRYPOINT”的特殊模式。

校验在 `core/startup`：最多 64 项参数，每项及启动程序最多 4096 字节，总计最多 32 KiB；拒绝 NUL，启动程序还拒绝空白及控制字符。参数按字节保留空格、Unicode、引号、换行和符号，不按 shell 语法拆分。桌面当前使用单行参数编辑器，多行参数可由 SDK 提供。

普通启动配置属于任务内容，会进入本机/Agent 操作记录与 Node 任务结果；敏感值继续使用独立的秘密环境变量通道。inspect 仍只返回参数数量，不新增原始参数字段。`startup` 缺省时不新增序列化字段，旧无覆盖命令的表示不变，旧日志仍可读取。

Apple 创建逻辑从 mutation dispatcher 拆入 `providers/apple/create`。CLI 选项全部位于校验后的镜像引用之前，启动参数位于镜像引用之后；指定程序使用单独的 `--entrypoint=...` 参数。遵循 [ContainerCreate 的 passthrough 参数声明](https://github.com/apple/container/blob/1.6.0/Sources/ContainerCommands/Container/ContainerCreate.swift#L48) 和 [Parser.process 的默认命令语义](https://github.com/apple/container/blob/1.6.0/Sources/Services/ContainerAPIService/Client/Parser.swift#L300)，不通过本机 shell 执行。容器启动程序可由用户显式指定为容器中的 shell，仍属于 ManageContainers 范围。

新增独立的 `startup_command` 诊断项与 `create_startup` 能力，共 18 项。Apple 1.6.0 要求基础创建契约和 `--entrypoint`；其他高级选项缺失不影响该能力，缺少该能力不影响默认创建。Agent 快照与诊断按当前设备的 ManageContainers/环境权限过滤。

## 桌面与 Node

桌面新增独立 `forms/startup` 模块：自定义/恢复镜像默认命令、可选启动程序、按顺序添加/移除参数。界面说明参数原样传递及任务存储边界。窄窗口沿用滚动表单、固定确认栏；空覆盖会留在表单显示错误，不提交任务。

```js
const task = await hub.execute({
  kind: 'create_container', environment: snapshot.environment,
  spec: {
    name: 'worker', image: 'example.invalid/worker:v1', cpus: 2, memory_mib: 512,
    options: { startup: { entrypoint: '/app/worker', arguments: ['--queue', 'daily jobs', ''] } },
  },
});
```

同 ID、同命令只返回原任务；调整参数顺序也属于不同命令，必须使用新 ID。增量 8 的恢复仍只查询原请求，保留完整启动配置，不重放创建；增量 9 的一次性秘密值仍不进入日志。Node 直接复用 Rust 公共校验，没有另写一套参数解析器。

## 验证与限制

- fmt、workspace/all-targets/all-features clippy（warnings denied）与 62 项 Rust 测试通过：desktop 8、core 13、Apple 19、management 5、remote 17。新增启动参数边界、无效覆盖与旧序列化形状检查；远程直连/中继控制端中断测试加入带启动配置的创建，恢复后仍执行一次、原配置完整保留。
- Node → bridge → 公共队列 → Apple CLI 模拟器验证四种默认/覆盖组合、镜像含多个 ENTRYPOINT 片段、空参数、空格、Unicode/换行、`--name`/`--entrypoint`/`--help`/`--` 作为容器参数、缺少能力时拒绝覆盖而允许默认创建、同 ID 并发去重/异文冲突、重启读取原配置且不重放。
- Node 远程创建、任务恢复、WebCrypto/WASM 只读设备拒绝创建和能力过滤通过；既有秘密输入、生命周期、镜像、日志与错误进程回归通过。
- 实际 GPUI 组件浅/深色 × 760×560、1040×700、1440×900 六组结构/交互检查通过，新增空覆盖阻止提交、程序/参数输入、空参数保留、移除、滚动及确认栏边界。不是原生像素、IME、系统剪贴板或 VoiceOver 验收；本轮未重复调用上一轮启动失败的电脑操作服务。
- 本机只读核对 `container --version` 和 `create --help`：CLI 为 1.6.0，声明了 `--entrypoint` 和剩余参数。没有实际创建或启动 Apple 容器，也未运行原始 Swift 参数解析器；源码、帮助与 CLI 模拟器证据不能替代真实运行时验收。

普通 Agent/bridge 构建拒绝 development 身份；开发 app 的 ad hoc 签名、bundle 身份及 19 个 Mach-O 文件内节均与编译产物匹配。打包后的 Node SDK/bridge 实际完成带启动配置和秘密变量的创建、脱敏检查及既有管理流程。

日志：`.local/validation-increment-10.txt`、`.local/apple-startup-contract-increment-10.txt`。开发包和打包验证记录位于 `dist/`、`.local/packaging-increment-10.txt`。未安装到 /Applications、初始化 Git、提交或发布。

M1 仍缺“创建后立即启动”的可选流程、服务启动入口、任务归档维护及真实运行时/原生视觉验收。Docker/Kubernetes、正式应用证明签发、公网/NAT 双主机、设备加入 UI、完整 Web、加密同步及恢复/根轮换仍未完成。
