# 原生、远程与公共模块开发记录

最新调整：[增量 22：紧凑容器列表](INCREMENT-22.md)、[增量 21：Stack 容器概览](INCREMENT-21.md)。环境表单、Docker Compose 与 Swarm 见 [增量 19](INCREMENT-19.md)；主机概览见 [增量 18](INCREMENT-18.md)。端口重建见 [增量 16](INCREMENT-16.md)。

各增量记录保留当时的验证结果与功能边界。`.local/` 中的本机日志、截图和运行时状态不随源码公开；README 使用的亮暗色原生截图保存在 `docs/images/`。

此前视觉调整：[增量 15：容器分页与双栏等高](INCREMENT-15.md)；按钮规范见 [增量 14](INCREMENT-14.md)。

2026-10-23。当前交付为可构建的开发实现，M1「快捷管理 0.1」与 M6「远程 Agent + Web」均未完成验收。逐项状态见 [第一阶段核查](../.agents/PHASE-ONE-AUDIT.md)，远程命令与协议见 [远程开发记录](REMOTE-DEVELOPMENT.md)。

本轮原型对齐、环境切换、镜像多选与验收见 [INCREMENT-13.md](INCREMENT-13.md)。上一轮视觉修复见 [INCREMENT-12.md](INCREMENT-12.md)。创建后启动与恢复见 [INCREMENT-11.md](INCREMENT-11.md)。启动命令见 [INCREMENT-10.md](INCREMENT-10.md)。一次性秘密注入见 [INCREMENT-9.md](INCREMENT-9.md)。跨层任务关联与恢复见 [INCREMENT-8.md](INCREMENT-8.md)。能力诊断、手动 CLI 路径、排序/标签名搜索与日志选区复制保留在 [INCREMENT-7.md](INCREMENT-7.md)。拉取进度/取消、高级创建与脱敏容器检查保留在 [INCREMENT-6.md](INCREMENT-6.md)。镜像与日志导出记录保留在 [INCREMENT-5.md](INCREMENT-5.md)。

## 当前实现

- 本机运行时服务启动：离线诊断中提供确认入口，公共 typed Command/持久化队列与 Node 共用；固定无交互参数，不安装内核、不提升权限，完成后核对服务。结果未知保留原任务，恢复只查状态。远程端明确拒绝该主机级动作。

- Rust core、management、runtime、Apple CLI provider、remote、GPUI desktop、Agent/Relay CLI，以及 Node bridge/SDK。公共边界与后续 N-API 入口见 [ARCHITECTURE.md](ARCHITECTURE.md)。Docker provider 与 Compose/Swarm 现有资源管理已实现，具体能力与真实 Engine 验收边界见增量 19；Kubernetes 仍未实现。
- 运行时诊断：22 项命令/参数能力、不可用原因、CLI 版本/路径和独立服务状态；未知版本禁止未适配写入，高级创建有独立门禁。桌面可保存可信本机 CLI 绝对路径，下次启动生效；Agent/bridge 支持本机启动参数配置，远程 RPC 不开放改路径。
- 全量容器列表、名称/镜像/标签名搜索、名称/状态/镜像稳定排序、状态筛选、配置详情；镜像列表、多选/全选、批量复制引用、占用核对后的批量删除、脱敏详情与删除，卷/网络只读列表。环境变量值在适配器入口丢弃。
- 启动、停止、二段重启；执行前核验完整资源键与创建身份，执行后查询实际状态。批量最多 32 项，冻结目标身份，逐项记录结果；停止确认提示 --rm 自动删除风险。
- 拉取镜像、基础创建、删除停止容器：桌面表单与公共 Command 统一校验，结果回到任务中心；Node 与远程 Agent 复用相同操作。创建支持名称/已有镜像/CPU/内存/多组端口、网络、命名卷、用户、工作目录和只读根文件系统，默认保持停止，可选创建后立即启动；删除确认可写层影响，拒绝运行中或旧 incarnation。
- 创建后立即启动复用 core/workflow；本机 JSON 和 Agent SQLite 均在启动前保存完整创建身份。失败保留 Container 结果，任务中心只启动同一 incarnation，秘密值无需重填；重启只核对、不重放创建，Agent 重新检查创建与生命周期权限。
- 创建支持启动程序/有序参数覆盖，保留镜像默认、仅替换参数或同时替换启动程序；独立能力门禁和公共边界校验，字面值传递。Apple 创建、核心启动配置及桌面表单各自拆成模块。
- 创建支持一次性秘密环境变量：只在公共命令/历史中保存名称，值经独立内存输入、加密远程通道和 Apple stdin 管道传递；不进入 argv 或临时文件。桌面值遮罩，Node 使用独立 secrets 参数；重启不补发输入，同 ID 重交值被拒绝。
- 拉取提供阶段进度与持久化取消请求；排队任务取消后不执行，运行中停止客户端后查询引用，保留未知结果。容器检查采用允许字段列表并校验原身份，桌面可刷新及复制 JSON。
- 镜像详情展示平台、digest、大小、环境变量名及引用关系；删除固定 reference/digest，拒绝仍被运行或停止容器引用的镜像，保留其他标签。CLI 清理失败记为 UnknownOutcome；同一 provider 的镜像写入与创建串行，外部 CLI 竞态仍存在。
- CPU/内存采样：Apple 1.6.0 结构化计数器、CPU 时间差、内存实际字节；首个 CPU 样本未知，失败保留最后值并标陈旧。采样按当前选择取消，旧 generation 不覆盖新资源。
- 任务先显示 Submitting，持久化 Queued 成功后才受理；有界并发 4 项、同资源串行，读取不被写入阻塞。失败项可单独重试；部分重启失败仅在实际已停止时重试启动。
- 桌面/Node 的新远程任务先持久化 Agent/所有者/原请求关联，再发送命令；重启后按原关联核对，最多自动排入 32 项，4 项并发、同资源串行。任务中心和 Node `reconcile(id)` 可继续查询未知结果，不自动重发 Submit。
- 本机旧记录启动恢复时，未执行的排队任务标为失败而不自动重放；Running 启停可按相同创建身份的实际状态核对，重启等不明确结果保留 UnknownOutcome。损坏/不可读的操作历史拒绝写入，保留原始证据。
- 应用/启动日志：最近 500 行、缓存 256 KiB、单行 4096 字符截断提示、ANSI 过滤、虚拟列表、横向滚动、搜索、复制筛选结果、冻结只读快照后选区复制、暂停/恢复。成功每 2 秒刷新，失败退避至 10 秒；选择切换取消旧读取并拒绝过期结果，日志不进入任务历史。
- 日志导出：桌面冻结当前筛选快照后选择新文件，0600、不覆盖已有目标；Node 返回相同格式的内容。包含资源身份、时间与有限尾部提示，路径不传给 Agent。
- 稳定远程环境 ID，容器资源键增加 provider/namespace，保留旧本机记录的默认 Apple/无 namespace 兼容。`--remote-profile` 复用同一桌面列表、详情、日志和生命周期 UI。
- Agent 端到端加密、直连/出站中继、单所有者原子绑定、Native/Web 独立设备授权/吊销、Native 租约续期、可靠受理与请求去重。协议中只开放已实现的管理动作，没有任意 shell API。
- Node 22+ ESM SDK 与 TypeScript 类型：受限 JSON stdio 子进程模式，支持 Apple/Demo/remote、任务提交/查询/等待、断线结果未知、持久化去重。JS 不持有私钥。使用示例见 [Node README](../packages/node/README.md)。
- 已确认的 v4 等距图标；共享主题 tokens。浅色/深色/跟随系统，宽度 <900 pt 收起侧栏文字，<1280 pt 详情占用主内容区，最小窗口 760×560 pt。

## 源码公开前验证（2026-10-23）

全工作区、全 feature 的 105 项 Rust 测试、Clippy（`-D warnings`）、格式检查和 8 组 Node 集成旅程通过。Node 管理旅程已按 Demo 不支持 Stack 的实际能力更新断言。17 个原型 JavaScript 文件通过语法检查，897 个第三方包的许可证清单与锁定依赖一致，`cargo deny` 许可证检查通过。

以上为本机隔离测试。此次整理未重复原生视觉、真实容器或公网远程验收；README 截图来自增量 22 的 GPUI/Metal 渲染记录。

## 历史验证（增量 17）

87 项 Rust 测试、全工作区 Clippy、Node 本机管理/异常退出/诊断/创建后启动及新增服务恢复旅程通过。亮/暗 × 三组窗口的 GPUI 交互检查通过；本轮像素、原型与预览记录见 [增量 17](INCREMENT-17.md)。真实本机服务和系统授权未操作。

## 历史验证（增量 13）

72 项 Rust 测试、fmt、全工作区 clippy 与六组 GPUI/Metal 交互/渲染通过。原生 Demo 中检查了亮/暗色、环境菜单、配置和镜像多选；开发包及独立预览已更新。66 张 PNG 与完整边界见 [增量 13](INCREMENT-13.md)。Node/Web 的跨端旅程仍是下节所述增量 11 的证据。

## 最近跨端完整功能验证（增量 11）

- Rust 1.99.1，GPUI Kit 0.8.0 / gpui-pre 0.3.9；WASM 使用本机带目标标准库的 Rust 1.99.0 工具链，未修改全局默认。
- fmt、全 workspace/all-targets/all-features clippy（warnings denied）、67 项 Rust 测试通过：desktop 8、core 13、Apple 19、management 8、remote 19。新增创建后启动成功/部分失败、检查点写失败、实际队列中断恢复、同名替换、双权限及创建期间吊销；保留启动配置边界/兼容、程序与参数覆盖、参数顺序去重及远程恢复；保留秘密输入限制/保真、错误脱敏、同 ID 冲突、断线恢复及 SQLite canary 检查；保留关联先落盘、工作任务中断恢复、直连切中继、过期历史查询、错绑定/记录拒绝、当前权限、Agent 重载保留未知和恢复取消语义；既有管理/远程安全测试继续通过。
- 实际 GPUI 组件的 6 组模拟窗口结构/交互检查通过：浅/深色 × 760×560、1040×700、1440×900。新增立即启动选择、含秘密声明的部分成功仅重试 Start、替代身份不显示重试及窄窗口按钮边界；保留启动程序/参数输入、空覆盖阻止提交、空参数与移除/滚动检查；保留秘密字段输入/遮罩的辅助功能值检查、表单滚动和提交分离；保留原任务核对按钮、核对中状态与关联保留；按钮不会提交新写任务。
- Node SDK → Rust bridge → 公共服务 → FakeProvider 的拉取/创建/启动/统计/日志/停止/删除通过，新增高级创建/脱敏检查/进度回调与结束任务取消；并发同 ID 去重、异文拒绝与重启不重放继续覆盖。
- 新增 Node → bridge → 私有 CLI 模拟器的绝对路径配置、离线诊断、逐项能力和写入拒绝；Node 远程与 Web 只读设备验证诊断环境映射、授权过滤。
- Node → bridge → Apple CLI 模拟器验证创建后启动、默认停止、失败保留身份、重启去重、单独启动及同名替换拒绝；缺少启动能力仍允许普通创建。
- Node → bridge → Apple CLI 模拟器验证镜像 ENTRYPOINT/CMD 默认与四种覆盖组合、独立参数传递、配置限额、独立能力门禁、去重/冲突及重启保留启动配置；不是实际容器执行。
- Node → bridge → Apple CLI 模拟器验证秘密输入字节摘要、空值/特殊字符、argv/DTO/日志不含值、排队输入丢失不重放及独立能力门禁；Node 远程与 Web 只读拒绝同步覆盖。
- Node 隔离历史夹具恢复丢失的本机最终结果：bridge 重启后按原关联读取 Agent 创建结果，即使目标已被后续删除也不会再创建。
- Node 异常进程夹具验证非零退出和关闭超时不报告成功，中断提交保留 operation ID；关闭错误不会被吞掉。
- 真实 TCP/WebSocket 直连和出站中继测试，以及 Node SDK 远程完整流程与 WebCrypto/WASM/WebSocket 互操作通过；新增远程进度与取消、只读取消拒绝、高级创建/容器检查，保留错误 pin、在线吊销及不导出根私钥。
- 单独验证普通 Agent 与 bridge 拒绝 development 身份；测试只使用临时夹具、CLI 模拟器和 FakeProvider，没有操作用户容器。
- 开发产物：`dist/ContainerHub.app`、`dist/bin/containerhub-agent`、`dist/bin/containerhub-bridge`、`dist/node`、`dist/browser-protocol`。已验证 app 的 ad hoc 签名、bundle ID 及 19 个 Mach-O 代码/数据节内容与编译产物相同；打包后的 Node SDK/bridge 实际完成 reconcile、运行时诊断、标签名读取、高级创建、容器检查、进度/结束任务取消、镜像删除及日志导出。协议版本不兼容、超长帧和 EOF 残帧均被拒绝。没有安装到 /Applications 或发布。
- 完整输出保存在 `.local/validation-increment-11.txt`，打包记录在 `.local/packaging-increment-11.txt`。增量 4 的记录保留；旧增量的真实 Apple 1.6.0 专用测试容器读取、旧身份拒绝、启动 → 重启 → 停止证据仍在 `.local/validation-increment-2.txt`；本轮没有重复该运行时旅程。

## 验收边界

- M1 尚缺任务归档维护；本机服务启动已在增量 17 补齐，真实运行时待验收。拉取目前为阶段进度，没有字节百分比。新增能力诊断/排序/日志流程已有代码与模拟验证，实际 Apple 运行时与系统保存窗口仍待验收。
- 正式 App Attest/应用凭据签发、真实公网/NAT 双主机、Safari/Chrome 存储与浏览器策略、完整 SvelteKit Web、云端加密同步、恢复备份/根轮换尚未完成。
- 增量 12 已实际检查原生 Demo 亮/暗色列表、详情、日志、表单、镜像与任务中心，并完成创建后启动；六组尺寸/主题保留 Metal 离屏 PNG。IME、VoiceOver、系统剪贴板完整行为、保存面板与真实运行时新增操作尚未验收。详见增量 12。
- Apple 公共 inspect/list 未提供可靠自动删除标记；停止与重启可能触发 --rm，消失的容器不会自动重建。CLI 超时结束客户端，不保证运行时未执行，需要核对结果。
- 本机历史使用原子 JSON + 实例锁；Agent 用 SQLite、安装标记与独立绑定检查点。恶意宿主整体快照回滚不在普通文件检查点的保证内。
- 本机列表显示最近 100 条，去重保留至 100000 条，满后拒绝新操作；归档工具待做。Node 本机与 Agent 日志通过持久化 receipt 关联；旧记录无法补造 request ID，Agent 自身中断且无充分证据时仍保留未知，不自动重放。
- 开发包只做 ad hoc 签名，未做 Developer ID、公证、App Attest 或自动更新。静态 PNG/icns 图标不是 Icon Composer 分层资产。
- 上游 block 0.1.6 的 future-incompatibility 提示仍存在，当前构建通过。

## 本机运行时升级状态

最近一次只读探测（增量 7）中，`diagnostics()` 通过本机 1.6.0 的全部 16 项 CLI 能力检查，独立服务状态报告不可连接。只读检查 `container system status --format json` 退出 1，报告 `command launchctl managername failed with status 153`。该次会话无法连接服务；增量 10 只读核对过 CLI 版本和 create --help 的 --entrypoint/剩余参数声明；本轮未重复 CLI 探测，未重复连接服务、启停运行时、改动授权或操作用户容器。

原安装由 Homebrew 管理，1.2.2_1 已升级到 1.6.0，并保留旧 keg。上一次成功核查时 CLI 和服务端均为 1.6.0。以下是升级阶段的最后成功核查快照，不是本增量的当前运行时检查。

升级保留了既有容器身份与镜像引用；部分容器的外置磁盘挂载仍需要用户在本机授予 macOS 可移动宗卷权限。详细本机状态保存在 gitignore 的 `.local/runtime-upgrade/`，不随源码公开。

## 日志适配来源

使用 Apple 1.6.0 公共 CLI 的 `logs -n 500` 和 `logs --boot -n 500`。参数及 stdout 行为按 [官方 ContainerLogs.swift](https://github.com/apple/container/blob/1.6.0/Sources/ContainerCommands/Container/ContainerLogs.swift) 核对；CLI 自身对非 UTF-8 日志的行为有上游限制，本地的容错解析不代表 CLI 会完整返回二进制日志。

端口重建记录：2026-10-23 [增量 16](INCREMENT-16.md)，包含公共重建命令、Apple CLI 契约、Agent/Node 接入及原型与原生视觉验证。
