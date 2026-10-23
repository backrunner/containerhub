# 增量 20 — Stack 操作界面精修

2026-10-23。设计稿 v8 与原生实现同步，范围为既有 Compose / Swarm 部署的查看与管理。

## 交付

- 208 pt 项目列表、紧凑服务表格，两栏等高且独立滚动。小窗口列表宽 164 pt，次要端口列移入展开区，常用操作保留。
- 32 × 32 pt 行操作按钮、8 pt 间距、红色停止按钮。移除放入更多操作菜单，执行前列出实际影响资源。
- 搜索 Stack、服务和镜像；Compose / Swarm 类型筛选；仅需关注筛选；服务多选、当前筛选全选和清除。标题栏操作针对已选服务，无选择时针对整个 Stack；单行及右键菜单始终针对该行。
- 服务表格显示镜像、运行状态、实例数、端口。容器健康检查与运行状态分开，未知或采集不完整不会显示为正常收敛。
- Compose 按需展开容器并跳转原有详情。Swarm 按需展开当前任务，历史记录可切换，每页 8 条，复制完整任务/节点 ID；Global 不再显示无意义的槽位 0。
- 副本编辑显示运行数和当前目标，0 副本使用红色“停止全部副本”确认，修改后清除过期的输入错误。

## 代码边界

桌面按列表状态、详情、服务行、容器实例、Swarm 任务、操作确认、副本表单拆分。服务运行状态与按服务范围生成操作计划放在 `containerhub-core`，可供其他客户端复用。

选择范围缺失或重复会报冲突，不能扩大为整组操作。确认框捕获资源身份和版本，入队前重新验证，保留现有权限、运行中操作、UnknownOutcome 和 32 项上限。未改变远程授权或任务队列协议。

仍不包含 Compose YAML 部署/up/down、Swarm 集群创建或完整 Stack 资源清理；Swarm 移除仅服务与任务，保留网络、卷、配置和秘密。此轮没有操作用户的容器或真实集群。

## 验收证据

- `cargo test --workspace --offline`：102 项通过。新增覆盖服务范围、缺失/重复选择、跨环境身份、历史失败任务、收敛状态、零副本和健康检查语义。
- 全 workspace / all-targets / all-features Clippy 通过。
- 760×560、1040×700、1440×900，亮/暗共 6 组真实 GPUI/Metal 离屏渲染和交互通过。覆盖单行操作与多选隔离、筛选全选、同名类型切换、镜像搜索、空结果、任务历史分页、过期版本、只读操作禁用及副本表单。
- 浏览器原型检查：批量确认范围、右键菜单、任务分页、零副本按钮、暗色和 760 pt 窄窗。窄窗两栏均为 356 px 高、底部 538 px，无横向页面溢出；行按钮均 32×32 px。原型 JS 全部语法检查通过，浏览器无捕获到的脚本错误。
- `dist/ContainerHub.app` 已重新构建并通过 ad-hoc 签名校验；没有安装、发布或修改发布版本。

原生图像使用真实 GPUI 渲染器和隔离的示例快照/操作队列，不是操作系统窗口截屏，也不是 Docker/Swarm 真机集群验收。浏览器图像为交互设计稿。

日志：`.local/stack-redesign-tests.log`、`stack-redesign-clippy.log`、`stack-redesign-ui.log`、`stack-redesign-package.log`。产物清单：`.local/stack-redesign-artifacts.json`。

截图：`.local/visual-review/stacks-v8-native-swarm-light.png`、`stacks-v8-native-compose-dark.png`、`stacks-v8-native-760.png`、`stacks-v8-native-scale-zero.png`；原型 `stacks-v8-swarm-prototype.jpg`、`stacks-v8-dark-prototype.jpg`、`stacks-v8-760-prototype.jpg`。
