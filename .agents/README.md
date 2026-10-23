# ContainerHub · 规划与设计

方案日期：2026-10-21；更新：2026-10-23。

最新设计：**v8.3 容器表格与 Stack 概览精修，减少圆点拼接和重复标签**。原型与原生已同步；布局见 [列表设计](LIST-REFINEMENT.md) 和 [Stack 设计](STACK-REDESIGN.md)，实现和验收见 [增量 22](../docs/INCREMENT-22.md)。

实现状态：**已实现增量 19：环境添加编辑、Docker provider、Compose 与 Swarm 现有部署管理及 Native/Agent/Node 公共接口；增量 18：环境管理原生实现、公共主机遥测、容器健康概览、编辑与右键管理，精简 UI 文案并同步 v6.1 原型；增量 17：本机离线服务启动、持久化任务与安全核对，桌面/Node 公共路径和 v5.5 原型同步；增量 16：端口映射弹窗与删除原容器后重建、危险确认、统一行操作间距及内存上限下标；增量 15：容器分页、当前页全选与双栏等高；增量 14：统计基线与按钮统一，设计稿 v5 同步，紧凑环境管理与底栏移除；按原型对齐导航/列表/详情，加入环境切换与镜像多选管理，完成原生 Demo 与六组亮暗色像素检查；补入可选创建后启动、持久化创建检查点与安全启动重试，保留一次性秘密注入，保留原请求关联、客户端重启恢复和人工核对；复用公共管理模块与 Node SDK；快捷管理 M1 尚未完成。原型使用已确认的 v4 等距玻璃容器图标。**

ContainerHub 从本机 Apple container 管理开始：用 Rust + GPUI 做一个启动快、操作直接、信息清晰的原生工作台，再逐步接入 Docker、Kubernetes、SvelteKit Web 和远程 Agent。

## 从这里看

| 文件 | 内容 |
| --- | --- |
| [PLAN.md](PLAN.md) | 产品范围、Portainer 对标、架构、阶段计划、风险和验收条件 |
| [第一阶段核查](PHASE-ONE-AUDIT.md) | M1 功能逐项差距、源码证据、回归结果与后续顺序 |
| [公共模块与 Node 接入](../docs/ARCHITECTURE.md) | core / management / runtime 分层、任务恢复、stdio 协议及 SDK |
| [远程 Agent 身份方案](REMOTE-AGENT.md) | 直连/中继、先来先绑定、Native/Web 设备授权、密钥同步与吊销；身份/传输已实现，完整同步待做 |
| [原生开发记录](../docs/DEVELOPMENT.md) | 已实现功能、真实生命周期证据、原生验证边界及本机升级快照 |
| [DESIGN.md](DESIGN.md) | 信息架构、视觉规范、关键流程、状态和 GPUI 落地约定 |
| [交互设计稿](design/index.html) | 本地浏览器预览；含浅色/深色、容器操作、详情、日志、新建、批量操作和异常场景 |
| [RESEARCH.md](RESEARCH.md) | 官方来源、本机只读核查、版本差异及尚待验证的假设 |
| [设计验证记录](design/VALIDATION.md) | 原型检查范围、截图与实际产品验证的边界 |
| [品牌展示](design/brand/index.html) / [资产说明](design/brand/README.md) | 独立品牌图形、应用图标、展示页与生成提示词 |

## 推荐决策

1. 首发面向 Apple silicon + macOS 26，采用独立签名、公证的 macOS 应用分发。
2. Rust 核心负责资源、操作、状态和错误；GPUI 只负责交互与展示。首版通过官方 CLI 适配 Apple container。
3. 以容器列表为默认首页。启停在行内，日志和终端从详情直接进入，长任务统一进入任务中心。
4. 分别定义「快捷管理首版」和「本机管理 1.0」；完整 Apple 管理包含镜像、构建、网络、卷、注册表、machine 和系统功能，实验性 k8s 单列。
5. 共享资源标识、操作与能力协议；Docker 保留 Compose 语义，Kubernetes 保留工作负载语义。
6. Web 使用 SvelteKit，复用 Rust 服务和协议；远程 Agent 承担所在机器的凭据和执行权限。

## 设计稿怎么体验

按 [本地预览方法](design/VALIDATION.md#查看方法) 启动静态服务后打开设计稿，所有数据均为示例。建议顺序：

1. 在容器行点击「停止」或「启动」，观察过渡状态和任务记录。
2. 点击容器名称，在右侧切换「概览 / 日志 / 配置（终端当前禁用）」。
3. 勾选多行，使用批量操作；将顶栏场景切为「部分失败」体验失败明细。
4. 点击「新建容器」，填写参数并创建示例记录。
5. 切换「镜像」「存储卷」「网络」「任务中心」，查看同一套信息层级。
6. 用顶部评审工具切换主题、空状态、运行时离线和窄窗口。
7. 进入「环境管理」，查看直连 / 中继 / 离线示例；点击健康分类、切换趋势范围，右键任意环境打开编辑表单。

原型只改变浏览器内的示例状态；刷新可恢复。它不连接本机 container、Docker 或 Kubernetes，也不读取凭据。

## 当前交付边界

已有公共 Rust core / management / runtime、Apple provider、GPUI 桌面、Agent/Relay、Native 远程客户端、浏览器协议模块和 Node stdio SDK。基础拉取/创建/删除/统计复用公共执行路径，桌面大文件已拆分；既有批量、失败重试、持久化受理与每资源串行继续保留。Agent 支持直连/出站中继、首次所有者绑定、设备授权/吊销、租约续期和持久化请求去重。运行方法和实际范围见 [开发记录](../docs/DEVELOPMENT.md)。

增量 17 的 87 项 Rust 测试、6 组尺寸/主题交互与 96 张 GPUI/Metal PNG 通过；服务启动、隔离 Apple CLI、Node stdio 与远程/WebCrypto/WASM/WebSocket 回归见增量 17。正式应用证明签发、真实公网/NAT、完整 Web/加密同步仍未验收；M1 的任务归档维护和真实运行时验收待补，Docker 部分能力与 Stack 管理已实现但尚未真实集群验收；Kubernetes 适配尚未实现。Apple container 系统磁盘授权继续由用户稍后处理，最近一次增量 7 服务检查返回 launchctl status 153，该增量未重复连接，没有操作用户容器。开发包已重新生成；项目现采用 Apache-2.0，并建立 Git 仓库。早期增量中的“未提交”记录描述当时状态，不代表当前状态；正式应用发布仍未完成。

早期原型对齐、环境切换和镜像多选见 [INCREMENT-13.md](../docs/INCREMENT-13.md)。上一轮视觉修复见 [INCREMENT-12.md](../docs/INCREMENT-12.md)。创建后启动与检查点见 [INCREMENT-11.md](../docs/INCREMENT-11.md)；启动配置历史见 [INCREMENT-10.md](../docs/INCREMENT-10.md)。

增量 9 的秘密输入通道与验证见 [INCREMENT-9.md](../docs/INCREMENT-9.md)。

增量 8 的原任务关联、恢复与取消语义见 [INCREMENT-8.md](../docs/INCREMENT-8.md)。

最新服务启动与验证见 [INCREMENT-17.md](../docs/INCREMENT-17.md)，端口重建见 [INCREMENT-16.md](../docs/INCREMENT-16.md)。

增量 19：添加环境支持 Docker context 或已绑定 Agent 导入；Stack 页面按 Compose / Swarm 区分同名项目，含 Swarm 副本编辑。容器编辑仍仅端口重建，完整配置编辑及 Stack YAML 部署入口待补。
