# 增量 7：运行时诊断、稳定列表与日志选区

2026-10-22。本轮继续补 M1 的日常管理能力，复用现有公共 Rust 模块；没有处理系统磁盘授权或修改用户容器。

## 运行时诊断

- core 增加 `RuntimeDiagnostics` 与 16 项 `RuntimeFeature`，分别描述已安装 CLI 的命令/参数契约、不可用原因、路径/版本和服务连通状态。即使服务离线，仍可显示 CLI 信息。
- Apple provider 读取版本、15 组公共 `--help` 和 `system status`。帮助检查并发最多 4、单项最多 2 秒，服务检查最多 3 秒；输出受既有进程上限约束。帮助缓存 30 秒，文件指纹变化或显式刷新重新检查，重叠刷新共享一次探测。
- 只匹配选项声明中的完整参数，不把说明文字或相似前缀当成支持。缺少命令/参数时对应功能不可用，provider 层也拒绝执行；高级创建独立于基础创建。
- 未适配版本只开放通过检查的读取；写操作仍限定 1.6.0，并保留原有身份检查和结果核对。帮助检查不是完整版本兼容性保证。
- `diagnostics()` 贯通桌面、bridge/Node 和远程 Agent。远程报告映射为 Agent 环境，并按当前设备的 Read/Logs/Lifecycle/ManageImages/ManageContainers 权限过滤；每次实际操作仍独立检查权限。

本机 Apple 1.6.0 已通过全部 16 项只读命令/参数检查，服务状态独立报告不可用；原始 `system status` 仍返回 `launchctl managername` status 153。未尝试启动服务或修改授权。命令依据为本机帮助输出与 [Apple 1.6.0 公共命令参考](https://github.com/apple/container/blob/1.6.0/docs/command-reference.md)。

## CLI 路径与列表

桌面设置可指定可信 Apple CLI 的绝对文件路径，保存前检查文件、执行权限和可识别的 Apple 版本；服务离线不阻止保存。成功后下次启动生效，也可恢复自动发现。主题/排序的晚到保存不会覆盖已确认的路径。显式路径失效时显示错误，不回退执行其他 CLI。

Node Apple 模式新增 `appleBinary`，Agent `serve` 与 bridge 新增 `--apple-binary`。这只是主机本地配置，不接受从远程 RPC 修改 CLI 路径。

公共 `inventory` 模块提供名称、状态和镜像排序，支持升降序及完整资源身份的确定性次级排序。桌面保存偏好；名称、镜像和标签名称可一起搜索。新增 DTO 字段 `label_names` 有旧数据默认值，标签值在 Apple 适配边界丢弃。

## 日志选区

日志面板的「选择文本」打开当前筛选结果的只读快照，支持拖选、Shift + 方向键、复制选区按钮及 ⌘C。选择期间后台刷新不会改动快照；中文与多字节文本保留。快照仍遵守原来的有界日志范围，不是完整历史，也不会进入任务记录。

诊断设置、日志选区和新增 UI 验证分别拆为模块；Node 与界面没有另写一套容器管理逻辑。

## 验证与交付

- fmt、workspace/all-targets/all-features clippy 通过；50 项 Rust 测试通过：desktop 8、core 9、Apple 19、management 2、remote 12。
- 新增离线诊断、缓存/刷新、未知版本写保护、缺失命令/参数不执行、显式失效路径不回退、路径保存顺序、稳定排序、标签值丢弃与旧 DTO 兼容测试。
- 6 组 GPUI 结构/交互检查通过：浅/深色 × 760×560、1040×700、1440×900。覆盖排序/偏好、离线报告、保存路径确认，以及只读中文选区和后台刷新时内容冻结；既有管理流程继续通过。
- Node 本机、私有 CLI 模拟器、异常子进程、远程直连/出站中继与 WebCrypto/WASM/WebSocket 互操作通过；包含只读授权的诊断过滤。模拟器与 FakeProvider 不操作用户运行时。
- 普通 Agent/bridge、开发 app、Node 包与浏览器协议重新构建。普通构建拒绝 development 身份；app 的 ad hoc 签名及 19 个文件内 Mach-O 节匹配构建产物；打包 SDK 实际调用诊断、读取标签及完成管理流程。
- 记录：`.local/validation-increment-7.txt`、`.local/packaging-increment-7.txt`、`.local/runtime-increment-7.json` / `.txt`。日志保留修正前的失败与最终通过结果。

电脑操作服务本轮仍启动失败，未取得原生截图。结构验证不能代替原生像素、IME、系统剪贴板、辅助功能或系统保存窗口验收。未安装到 /Applications、初始化 Git、提交或发布。

M1 仍缺秘密注入等完整创建能力、跨层任务恢复/维护与服务启动入口；真实 Apple 旅程和原生验收仍待完成。Docker/Kubernetes provider、正式应用凭据签发、真实公网/NAT、设备加入 UI、完整 Web、加密同步及恢复/根轮换也尚未完成。建议接下来先做可恢复的远程任务映射与秘密注入，再推进 Docker，最后按独立 workload 语义接 Kubernetes。
