# 增量 5：镜像详情、删除与日志导出

2026-10-22。本轮继续补齐 Apple 快捷管理，桌面、Node、Agent 共用 core / management / provider，未新增 Docker/Kubernetes provider。

## 镜像管理

- 公共 `ImageKey` 固定 environment、provider、完整 reference 和 digest。`inspect_image` 返回创建时间、媒体类型、各平台 digest/大小、环境变量名称、容器引用及其他同内容标签。
- 只传允许的字段：镜像环境变量值、命令参数、标签值不会交给 UI、Node 或远程客户端。复制详情也是同一份脱敏结构。
- `Command::DeleteImage` 走持久化任务、去重、权限检查和结果核对。执行前重新读取标签及引用；旧 digest、无法核对的引用，以及运行或停止容器的引用均拒绝删除。
- 按完整 reference 删除一个标签，不用 digest 去匹配一批标签，不使用 `--all` 或 `--force`。桌面先显示引用关系，再确认删除；同内容其他标签保留。
- 单个 Apple provider 内，拉取、创建和镜像删除串行，避免自己的创建与镜像删除交叉；读操作和其他容器生命周期不受这把锁阻塞。外部 CLI/其他进程不在锁内。公开 CLI 不支持带预期 digest 的原子条件删除，因此仍有外部重标记/新建容器的检查与执行竞态。
- Apple CLI 删除引用后还执行孤立数据清理。即使引用已消失，只要命令失败，就保留 UnknownOutcome，不把清理失败当作完整成功。其行为依据 [Apple 1.6.0 ImageDelete](https://github.com/apple/container/blob/1.6.0/Sources/ContainerCommands/Image/ImageDelete.swift)。

原始 JSON 结构依据 [ImageResource](https://github.com/apple/container/blob/1.6.0/Sources/ContainerResource/Image/ImageResource.swift)；实际展示使用自己的允许字段模型，不输出完整运行时 JSON。`inspect_image` 使用 Read 权限，删除使用既有 ManageImages 权限；只读设备仍可查看详情，不能删除。旧能力快照缺少新增字段时默认关闭新入口。

## 日志导出

公共 `LogSnapshot::export` 生成 UTF-8 内容与安全的建议文件名。元数据包含完整资源身份、来源、采样时间、筛选条件、导出/快照行数和本地截断标记，明确是有限尾部，不是完整历史。保留最近 500 行、256 KiB 及已有控制字符过滤边界。

桌面先冻结当前筛选快照，再打开系统保存窗口；即使期间刷新日志或切换容器，也保存原先选中的内容。使用临时文件及不覆盖落盘，在 Unix 上权限为 0600。目标已存在（包括符号链接）时拒绝覆盖；取消保存窗口不创建文件。

Node `exportLogs(key, {source, query})` 获取新日志快照后调用相同导出格式，返回内容，由 Node 调用方选择本机文件位置。文件路径不会发给远程 Agent；Agent 仍执行原有 Logs 权限检查。SDK 示例见 [Node README](../packages/node/README.md)。

## 验证与边界

- 36 项 Rust 测试：desktop 7、core 7、Apple 10、management 1、remote 11；fmt / all-targets / all-features clippy 通过。
- Apple CLI 模拟器覆盖停止容器的跨标签引用、重标记后旧 digest 拒绝、仅删除选中标签、清理失败、命令参数及结果核对；镜像详情测试确认环境值/命令/标签内容不泄露。
- 日志测试覆盖筛选、快照冻结、来源/时间/截断信息、文件权限、已有目标及符号链接保护。
- 6 组 GPUI 结构/交互检查覆盖亮/暗色 × 760×560、1040×700、1440×900，新增镜像详情、复制、引用禁用、确认删除、过期结果拒绝与日志导出入口。列表与详情分开保存滚动状态，覆盖从列表底部进入详情后操作仍可见。无界面测试平台只模拟保存窗口取消；真实系统保存窗口仍未验收。
- Node 本机及经 Agent 的镜像 inspect/引用保护/删除/日志导出通过；只读 Web 设备不能删除镜像。既有加密直连/中继、吊销与 Node 异常退出继续回归。
- 本机运行时只读检查仍返回 `launchctl managername` status 153；未操作用户容器或镜像。没有将 CLI 模拟器、Headless GPUI 或 Node 网络测试当作真实 Apple 运行时、系统窗口视觉或公网验收。

输出：`.local/validation-increment-5.txt`、`.local/packaging-increment-5.txt`、`.local/runtime-increment-5.txt`。开发包重新生成到 `dist/ContainerHub.app` 与 `dist/node`，未安装、签发正式凭据或发布。

M1 下一批仍包括拉取字节进度/取消、高级创建选项、容器脱敏 inspect、日志选区复制、能力发现及恢复维护；生产应用证明、真实公网/NAT、完整 Web/云端同步仍是独立待完成项。
