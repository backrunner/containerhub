# 增量 18 · 原生环境管理与主机遥测

2026-10-23。环境管理 v6.1 已实现于 GPUI 原生代码，并同步精简 HTML 原型中的重复说明。

## 原生交互

- 左侧环境列表搜索、单击查看、右键查看/切换/编辑/复制/刷新，远程可确认移除。编辑和菜单始终针对点中的行；查看目标与当前工作环境独立。
- CPU、物理内存、系统盘和非回环网卡的真实采样与图表；1/5 分钟范围、暂停/继续、2/5/10 秒间隔。历史限 5 分钟和 151 个样本，离开页面或切换工作空间停止主机采样，迟到响应按 generation 丢弃。
- 容器运行状态、健康比例和分类明细弹窗。未配置、不支持、未知与停止状态不会被计为健康。Apple adapter 没有健康检查数据时明确显示不支持。
- 编辑名称、备注、采样间隔、本机 CLI 路径及远程直连/中继/自动策略。名称/地址/间隔校验；取消不写入。连接参数写入原子替换的 `environment-preferences.json`，只含公开配置，不含私钥。
- 远程连接展示目标、中继地址、原 Agent 指纹与绑定时间；请求耗时包含认证，最近采样按本地接收时间计算。不伪造长连接时长、心跳或实际自动路由选择。
- 修改地址使用原 profile 的凭据和 pin，身份不匹配拒绝连接。自动模式仅在直连传输建立失败时回退中继，身份验证失败不回退。进行中或待核对任务存在时禁止改连接。
- 移除只影响此设备列表，不删除容器、不解除 Agent 所有者绑定；当前环境移除后回到 Demo。

## 模块边界

| 模块 | 职责 |
| --- | --- |
| `crates/core/src/host.rs` | HostSnapshot、健康枚举和汇总契约 |
| `crates/host` | 无进程枚举的只读 OS 采集器；sysinfo 0.39.6 |
| `crates/runtime` | 原身份连接配置校验与 ClientSlot 热替换 |
| `crates/remote` | Read 授权后的 HostSnapshot RPC、pin/环境核对与自动回退 |
| `apps/desktop/src/backend/environment.rs` | 可取消的所选主机采样；容器/主机错误分开处理 |
| `apps/desktop/src/environments/*` | 图表、主机信息、容器健康与连接展示 |
| `apps/desktop/src/workspaces/*` | 目录、独立查看、持久化、编辑与行菜单 |
| `apps/bridge`、`packages/node` | `hostSnapshot()`，同一 Rust 契约 |

## 验收

```sh
rtk proxy cargo fmt --all -- --check
rtk proxy cargo clippy --workspace --all-targets --all-features --offline -- -D warnings
rtk proxy cargo test --workspace --all-features --offline
rtk proxy cargo build -p containerhub-bridge --offline
rtk proxy node tests/node-diagnostics.mjs
rtk proxy cargo run -p containerhub --features ui-validation --offline -- --validate-ui
rtk proxy python3 scripts/package-dev.py
```

测试覆盖真实只读 OS 初始/后续样本、缺失与非法值、健康计数、Agent 命名空间、Read 授权、错误 pin、隔离 loopback 直连/中继/回退、Node 离线运行时下读取主机信息。原生交互验收覆盖查看与切换分离、右键行编辑、取消、名称/采样持久化、暂停和双栏等高，并保留原有管理/重建/服务启动回归。

Metal 原生渲染验收为 760×560、1040×700、1440×900 × 亮/暗色，含环境页与远程编辑 modal。证据在 `.local/ui-validation/*-environments.png`、`*-environment-edit.png` 和 `environment-context-menu.png`。截图里的遥测/健康为显式 Demo/fixture，真实主机采集另由 OS/Node 测试验证。

开发包仍是本地 ad-hoc 签名；`demo-preview` 是显式 Demo 预览构建，普通包不启用此 feature。没有发布、推送、操作真实容器或代办首次 macOS 授权。公网 WSS、真实远程主机与跨设备密钥同步没有因此被视为已验收。

最终结果：90 项 Rust 测试、fmt、全目标全 feature Clippy（`-D warnings`）、Node diagnostics + hostSnapshot、六组原生渲染与交互检查通过。普通开发包和 Node 本地包已刷新。原生截图由 Metal offscreen renderer 生成；本轮 CUA 无法取得独立预览应用的系统窗口（`cgWindowNotFound`），不将进程启动当作系统窗口视觉验收。
