# 设计稿验证记录

对象：同目录的 `index.html`、`style.css`、`app.js`、`icons.js` 与 `brand/`。

## v8.3 容器列表精修（2026-10-23）

主列表使用 56 pt 双行条目、CPU/内存独立列和固定操作区。列显隐依照列表实际可用宽度；Stack 展开区改为共享表头，窄窗保留字段标签。左侧列表的服务数与运行/关注状态左右对齐，相关界面去掉圆点拼接文案。

原生三尺寸 × 亮暗色 GPUI/Metal 交互与渲染检查、桌面 Clippy 和 17 个原型 JS 语法检查通过。Edge 核对 760/1040/1280/1440 px、亮暗色、详情开合、跨页选择；行高 56 px，按钮间距 8 px，无页面横向溢出，两栏底边相同。已查看原生与浏览器截图并更新开发包。详细证据及真实集群验收边界见 [增量 22](../../docs/INCREMENT-22.md)。

## v8.2 两行 Stack 列表 · 2026-10-23

左侧项目列表压为两行：名称＋类型；服务数量＋需关注／运行状态。每项 56 pt，行内间距 4 pt、项目间距 2 pt；摘要与名称左对齐，长名称省略，类型标签保持宽度。原型和 GPUI 代码同步。

既有三种尺寸 × 亮暗色原生交互与 Metal 渲染检查通过；桌面 Clippy 通过。Edge 核对 760×560 浅色和 1440×900 深色的高度、选择切换、无横向溢出。原生紧凑浅色和宽窗深色均已查看。截图与日志见 [增量 21 补充记录](../../docs/INCREMENT-21.md#v82-follow-up--compact-project-rail)。

## v8.1 容器快速概览 · 2026-10-23

服务展开区统一为 Compose / Swarm 容器概览，移除任务历史与当前/期望任务状态。展示容器身份、节点、实际状态/健康、CPU、内存 / 小号上限和端口；复制容器 ID、跳转详情及超过 4 个容器分页。未知/过期数据为“—”，未采集详情不可跳转。宽窗横排、窄窗两行；窄窗另外保留 Swarm 服务发布端口。

104 项 Rust 测试、Clippy、三种尺寸 × 亮暗色 GPUI/Metal 渲染和交互检查通过。Edge 核对浅色 Swarm、深色 Compose、760×560 双栏等高和无横向溢出、详情跳转和空概览。检查中修复了原型跨页重复绑定 Stack 控件的异常；v8.1.1 静态资源版本复验无新增错误。设计规范见 [Stack 设计](../STACK-REDESIGN.md)，截图、日志及边界见 [增量 21](../../docs/INCREMENT-21.md)。

当前 Docker 适配器尚未提供实时容器资源采样。截图为隔离示例；原生图片来自真实 GPUI/Metal 渲染，不代表真实 Docker/Swarm 集群验收。

## v5 布局与控件精修 · 2026-10-23

沿用 v4 图标，更新统计基线、Logo 边框、环境切换、设置/刷新按钮、红色停止、列表操作间距、紧凑环境管理与镜像多选。全局底栏已删除。详细实现和证据见 [增量 14](../../docs/INCREMENT-14.md)。

三种尺寸（760×560、1040×700、1440×900）× 亮暗主题，对容器、环境、镜像页面检查无页面横向溢出、可见图标按钮宽高相等、四项统计完整。Edge 实际查看宽窗浅色、中窗镜像多选、窄窗深色；原生侧另做 66 张 GPUI/Metal 截图和真实 Demo 窗口检查。原始批准截图未覆盖，v5 原型和原生结果由 `.local/visual-review/index.html` 对照。

原型新增 `refinement.css`、`environments.js`、`images.js`。`?preview=1` 可隐藏评审外框，以应用逻辑尺寸检查；`theme=dark`、`page=runtime` 可直接进入对应评审状态。所有动作仍仅为示例模拟。

## v4 等距图标更新 · 2026-10-22

当前图标为内置 image_gen 生成的三枚蓝色玻璃容器等距堆叠，1254 × 1254 不透明 PNG。侧栏、评审栏与 HTML favicon 引用已更新。材质与提示词见 [品牌资产](brand/README.md)。

在本机 Edge 实际打开 HTTP 原型，人工查看浅深背景与 18 / 32 / 40 / 64 px 展示，全部 10 张图像加载成功。展示页在浏览器默认宽度与 390 px 宽度均无横向溢出；窄屏下两组小尺寸图标未超出卡片。已恢复浏览器默认视口。

应用内浅深主题均已查看，侧栏与评审栏实际背景图引用 v4，显示尺寸为 36 px 与 18 px。主题切换成功，页面无横向溢出，浏览器未记录 warning/error。本轮只更新图标与品牌展示，未重新执行历史完整交互回归。

当前真实浏览器截图：[图标展示](brand/brand-preview-v4.jpg)、[390 px 展示](brand/brand-preview-v4-narrow.jpg)、[浅色应用](brand/brand-in-ui-v4-light.jpg)、[深色应用](brand/brand-in-ui-v4-dark.jpg)。

生成过程中的透明外缘探索存在杂点，未采用；最终采用完整方形母图，由网页圆角裁切。深色背景使用同一浅色图标；未验证原生 Dock、Dark / Mono 图标变体或动态 Liquid Glass，也未制作 Icon Composer 分层工程。

## v3 品牌图形更新 · 2026-10-22

以 image_gen 绘制的独立图形替代 CH 字形。当前资产与提示词见 [品牌资产](brand/README.md)。侧栏、评审栏和 favicon 均已更新；实际浏览器验证浅深色应用中的呈现、品牌展示页全部图片加载与 18 / 32 / 40 px 小尺寸显示。品牌展示页 1280 px 宽度无横向溢出。

历史 v3 品牌截图：[展示页](brand/brand-preview-v3.png)、[浅色应用](brand/brand-in-ui-light.png)、[深色应用](brand/brand-in-ui-dark.png)。下方 v2 截图保留为历史视觉记录，包含已停用的 CH 标识。此次没有改动容器操作逻辑，也没有重复宣称历史完整交互检查已重新执行。

## v2 视觉更新验证 · 2026-10-22

通过 Codex 内置浏览器实际打开本地 HTTP 预览，检查钴蓝主题、CH 标识、表格和详情对齐。已重生成下方全部截图。默认视口为 1280 × 720，截图使用浏览器整页捕获；窄窗口按钮将应用外框收至 1080 px。两种外框均无页面横向溢出。窄窗口中标题与摘要左边界同为 303 px，CPU 与内存趋势图顶部同为 547.59 px。

本轮回归通过：深浅主题切换、单容器停止和启动完成后的可用按钮状态、名称搜索、停止状态筛选、详情日志、新建抽屉打开/取消、批量停止部分失败、运行时离线禁用创建。实际主按钮色与 CSS token 一致，SVG 品牌资产可见；浏览器未记录控制台错误，JavaScript 语法检查通过。

本轮是视觉与相关交互回归，下列完整 18 类交互与 6 个视口检查属于 v1 历史记录，没有宣称本轮全部重跑。

## v1 历史验证 · 2026-10-21

通过本机 Chromium / Playwright 打开实际 HTML 页面，执行 18 类交互检查：

1. 单容器停止与启动，过渡后显示最终状态。
2. 搜索、无结果和清除筛选。
3. 按运行/停止状态筛选。
4. 日志 Tab 与暂停跟随。
5. 配置 Tab 的敏感示例值遮蔽。
6. 终端模拟输入与固定示例响应。
7. 重复容器名称阻止提交。
8. 已占用示例端口阻止提交。
9. 创建新记录、删除确认和删除结果。
10. 批量操作部分失败，保留逐项结果。
11. 只重试失败任务。
12. 运行时离线时禁用写操作。
13. 重新连接恢复可用状态。
14. 无容器空状态。
15. 镜像拉取模拟完成后更新列表。
16. 存储卷创建模拟。
17. 命令面板搜索、Enter 执行和关闭。
18. 环境弹窗正确标明后续 Docker/Kubernetes/Agent 尚不可连接。

检查宽度为 1440、1080、1040、768、390、360 px，未发现页面横向溢出或控件超出窗口左右边界。桌面使用独立的列表与详情滚动；移动宽度仅用于后续 Web 布局探索，不表示已经支持移动产品。

检查过程中发现命令面板 Enter 的默认按键行为会重新打开弹窗，已修复并重新验证。最终交互检查通过，页面无 JavaScript 错误。语法检查通过；图标为本地 Lucide 子集，带 [许可证](LUCIDE-LICENSE.txt)，没有 CDN 或 API 请求。

机器可读结果：[validation-results.json](validation-results.json)。

## 实际渲染截图

| 截图 | 场景 |
| --- | --- |
| [containers-light.png](containers-light.png) | 浅色默认容器列表 |
| [containers-dark.png](containers-dark.png) | 深色默认容器列表 |
| [container-logs.png](container-logs.png) | 日志检查器 |
| [create-container.png](create-container.png) | 新建容器抽屉 |
| [runtime-offline.png](runtime-offline.png) | 运行时离线与禁用操作 |
| [tasks-partial.png](tasks-partial.png) | 部分失败的逐项任务结果 |
| [containers-narrow.png](containers-narrow.png) | v2 1080 px 应用窄窗口 |

v2 已人工查看浅色、深色、新建和窄窗口截图；日志、离线和部分失败同时保存实际渲染截图。截图由浏览器渲染原型生成，未使用图片生成工具伪造产品画面。

## 本次没有验证的内容

Rust 编译、真实 GPUI 窗口、VoiceOver、中文 IME、原生 PTY、实际容器启停/构建/网络/卷、真实日志流、资源统计精度、签名安装、远程连接和应用性能。它们在 [实施计划](../PLAN.md) 中有独立验收门槛。

原型中的启停、创建、重试、终端和断线全部是内存状态模拟，刷新恢复默认数据。设计验证不能作为应用实现完成的证据。

## 查看方法

文件入口为 [index.html](index.html)，依赖同目录本地 CSS、JS、功能图标与品牌 PNG。建议在仓库根目录运行本地静态预览（遵循本仓库 RTK 约定）：

```sh
rtk proxy python3 -m http.server 8765 --bind 127.0.0.1 --directory .agents
```

然后访问 `http://127.0.0.1:8765/design/`。这只提供设计文件，不连接任何运行时。

## 2026-10-23 分页补充验证

设计稿与原生均加入分页、每页数量和当前页全选；末页保留双栏同高。Edge 验证 5 条/页的两页切换、跨页保留选择、筛选回首页；1440×900 原型列表/详情上下边界均为 226/878 px。三种桌面尺寸无横向溢出。原生六组亮暗色 Metal 交互与截图、实际 Demo 窗口验证单独记录于 [增量 15](../../docs/INCREMENT-15.md)，不将浏览器验证当作原生证据。

## 2026-10-23 · v5.3 端口编辑

新增端口重建弹窗、统一行操作 8 pt 间距及内存斜线下标。Edge 实测端口 0 被拒绝，改为 3001 后更新映射且仍为 8 个示例容器。亮暗色 × 760×560 / 1040×700 / 1440×900 均无横向溢出，弹窗在视口内完整呈现。

最终原型截图：`.local/visual-review/prototype-*-ports-modal.png`；尺寸记录：`.local/visual-review/ports-layout.json`。原生端口表单与概览截图：`.local/ui-validation/*-ports-modal.png`、`*-ports-overview.png`。完整执行及验证边界见 [增量 16](../../docs/INCREMENT-16.md)。

## 2026-10-23 · v5.4 收起按钮与动画

Edge 实测亮暗色 × 760×560 / 1040×700 / 1440×900：按钮均为 32×32、1 px 描边，与容器图标顶部对齐、左右内距相等，无横向溢出；六种布局均可收起并重新展开。记录：`.local/visual-review/collapse-prototype-layout.json`，截图：`prototype-collapse-light.png`、`prototype-collapse-dark.png`。

原生 Metal 验收覆盖相同六种布局：中间帧详情内容宽度不变、详情列与间距缩小、列表同步扩展；完成后释放日志订阅，重新选择取消关闭，减少动态效果立即关闭。84 张原生状态截图通过生成，详情按钮见 `.local/ui-validation/*-ports-overview.png`。桌面 14 项测试、Clippy 和原型 JS 语法检查通过。

重新构建并启动独立 Demo 预览，实际窗口确认描边、对齐、收起后的列表扩展以及点击容器行重新展开；截图 `.local/visual-review/native-collapse-button.png`。未连接真实容器运行时。


## 2026-10-23 · v6.0 环境管理重设计

本轮仅更新 HTML/CSS/JS 原型与设计规范；未修改、构建或验收 GPUI / Rust / Agent，不连接真实运行时。原型入口：`/design/?preview=1&page=runtime`，参数 `environment=local|studio|build|qa` 可直接查看相应样本，`theme=dark` 查看暗色。

Edge 实际交互验证：

- 选中构建节点后，右键开发工作站并保存新名称：只改开发工作站；右侧仍是构建节点，顶部仍是演示工作空间。查看与切换分离。
- 拒绝重名、http 目标地址及中继策略下的空地址；错误显示在对应字段下，带 aria-invalid 与错误关联。有效编辑保存。修改本机名称后取消保持原值；本机 CLI 示例路径保存后在连接详情更新。
- 改为仅中继后，目标输入禁用，保存显示待验证、切换禁用，Agent 指纹不变；模拟检查通过后恢复连接。测试节点检查超时，绑定身份步骤显示尚未执行。
- 未配置健康检查过滤得到 2 个运行中、未配置检查的容器，未混入已停止或健康容器。五个环境样本的总数、健康计数和唯一容器名通过独立一致性检查。
- 示例曲线持续变化；暂停后路径保持不变；5 分钟范围切换有效；离线冻结曲线，标签为断开时，健康为待更新，保留上次记录。
- 搜索无结果可清除；Shift+F10 打开目标行菜单；Escape 关闭并恢复到触发行。760×560 菜单底边 552 px，未越出视口。
- 760×560 编辑弹窗边界 110/16/650/544 px，保存按钮始终在视口内，内容内部滚动；暗色字段错误可见。
- 直连和中继路径、备用目标地址、绑定指纹对话框均检查；本机离线「启动服务」仍可打开确认、执行时禁用、完成后恢复连接，未知结果只提供核对。所有动作都是内存模拟。

亮 / 暗色 × 1440×900、1040×700、760×560：六组均无页面或详情横向溢出，列表和详情上下边界一致；宽 / 中等窗口四列资源图表，最小窗口两列。滚动连接详情不会带动环境列表。JS 11 文件语法检查通过，最终页面未记录浏览器 error / warn，原有容器页导航回归通过。

证据保存在仓库 `.local/visual-review/`：

| 文件 | 内容 |
| --- | --- |
| `environment-v6-layout.json` | 六组尺寸、主题、两栏边界与溢出检查 |
| `environment-v6-{light,dark}-{1440,1040,760}.png` | 六组实际浏览器渲染 |
| `environment-v6-edit-light.png` | 编辑表单完整布局 |
| `environment-v6-dark-edit-760.png` | 暗色小窗口固定操作栏与字段错误 |
| `environment-v6-relay-light.png` | 中继路径、目标地址和身份信息 |
| `environment-v6-context-light.png` | 非选中环境的右键菜单 |
| `environment-v6-offline-light.png` | 离线历史数据及健康待更新 |

这些截图来自实际 Edge 渲染，不是图片生成。模拟检查通过不代表安全握手、实时遥测采集、WAN、持久化、密钥同步或 Native 功能通过验收。原生开发状态继续参见增量 17。

## 2026-10-23 · v6.1 原生落地

精简列表提示、表单页脚和健康解释文案，移除未实现的自动连接选项与不适用于按请求连接的断开按钮。请求耗时替代 RTT、最近响应替代心跳。原生环境页、健康明细和编辑表单已实现；模块与验证见 `docs/INCREMENT-18.md`。本轮原生 Metal 六尺寸/主题验收产物位于 `.local/ui-validation`，HTML 预览仍为示例数据。


## v7.1 — Environment forms / Compose / Swarm (2026-10-23)

Native implementation and evidence: [Increment 19](../../docs/INCREMENT-19.md). Docker environment entry, same-name Compose/Swarm selection, service/task details, replica modal and environment add/edit forms are synchronized with native. Forms use fixed-size actions, 8 px button gaps and 32 px controls. Preview actions only mutate sample state.

Native screenshots in `.local/ui-validation` are GPUI/Metal offscreen renders of production components with isolated fixture data, in both themes and three window sizes. Browser screenshots are in `.local/visual-review`. Neither establishes a working real Docker/Swarm cluster. Docker telemetry, full configuration editing and YAML deployment are not represented as implemented.

## v8.0 — Stack table and scoped operations (2026-10-23)

See [interaction spec](../STACK-REDESIGN.md) and [increment 20](../../docs/INCREMENT-20.md).

Native and prototype now share the rail / table / expandable instance structure, fixed action widths, type and attention filters, service selection, context menu, task history pagination and replica form. 102 Rust tests, all-target/all-feature Clippy and all six native viewport/theme suites passed. Browser checks covered 760×560 and 1440×900, both themes, two-service confirmation scope, history paging and zero-replica danger action. At 760 pt both panes measured 356 px high and 538 px bottom; action buttons were exactly 32×32 px, with no page overflow.

Native captures are GPUI/Metal offscreen frames with isolated fixtures, not OS window screenshots or live-cluster verification. Browser captures are interactive prototype fixtures. Dedicated v8 captures are under `.local/visual-review/stacks-v8-*`; local development bundle was rebuilt and signature verified. No release or installation performed.
