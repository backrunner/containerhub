# 增量 12：原生视觉检查与布局修复

2026-10-22。电脑操作服务已恢复，本轮实际启动独立的 ContainerHub Preview.app（`--demo`），检查原生窗口，并修复视觉缺陷。全部交互使用 FakeProvider；未启停 Apple 服务或操作用户容器。

## 修复

- 虚拟列表的容器行原先按内容收缩，造成行背景未铺满、表头和内容错位。现在行宽填满列表，状态、资源、操作列使用一致宽度并禁止压缩。
- 两行记录从 74 pt 收紧为 60 pt，名称与镜像之间改为较小间距。
- 拉取、新建、刷新统一为右侧紧邻的操作组，新建使用主按钮样式。离线时禁用拉取/新建入口，保留刷新；避免进入当前无法提交的表单。
- GPUI 组件有独立按钮主题 token，单独修改 primary 不会改变原本黑白的主按钮。现在同时映射按钮、选中态、输入边界、焦点、文本选区和弹出层；主按钮悬停/按下采用不透明深蓝，避免透明叠色降低白字对比度。
- 六组布局检查新增整行宽度、状态列对齐、按钮相邻间距和组件主色断言。像素场景收集拆到 `ui_validation/visuals.rs`。

## 原生窗口证据

通过电脑操作工具实际查看 1440×900 pt 开发窗口的浅/深色列表、选中详情、日志、创建对话框、镜像资源和任务中心。macOS 窗口缩放动作也执行成功；没有把拖动边框的尝试当作精确尺寸验收。

实际点击创建表单，输入 `visual-review-nginx` / `nginx:alpine`，勾选创建后立即启动并提交。容器数从 8 变为 9，详情显示运行中，任务中心显示“创建并启动容器 visual-review-nginx / 已完成”。这是原生 UI → 公共队列 → Demo provider 的旅程，不是 Apple 容器运行证据。

## 可重复的像素验收

亮/暗色 × 760×560、1040×700、1440×900 pt，使用实际 GPUI/Metal 离屏渲染。每组保留列表、日志、部分创建结果恢复、离线提示、基础创建、高级创建、启动配置七种状态，共 42 张 PNG，位于 `.local/ui-validation/`。尺寸为逻辑 pt，PNG 使用平台像素倍率。

自动化场景采用测试上下文的 reduce-motion 模式，使对话框入场动画先稳定，再发送快速模拟点击；否则按钮可能在 pointer-down/up 两帧间移动，导致偶发未提交。相邻对话框也先结束前一个关闭事件。此设置只存在于 ui-validation，不改变正式应用，也不代表已验收系统 Reduce Motion 或动画质量。

运行命令：

```sh
rtk proxy cargo run -p containerhub --features ui-validation --locked --offline -- --validate-ui
```

完整本轮输出保存在 `.local/validation-increment-12.txt`。

验证包括 fmt、workspace/all-targets/all-features clippy（warnings denied）、桌面 8 项 Rust 测试、完整六组 GPUI 交互及像素场景。上一增量的 67 项 Rust/Node/远程矩阵记录保留在增量 11，本轮没有把它们算作重新执行。

开发包重新生成到 `dist/ContainerHub.app`，ad hoc 签名验证通过。独立 Preview 使用同一开发程序及 Demo 启动包装器；没有安装到 /Applications、初始化 Git、提交或发布。

## 剩余边界

本轮完成上述原生 Demo 视觉检查和多尺寸像素检查，尚未覆盖 macOS IME、VoiceOver、完整系统剪贴板/保存面板、系统外观自动切换或全部动画。真实 Apple 新功能运行旅程仍待授权后的专用测试资源验收。

M1 仍缺服务启动入口和任务归档维护。Docker/Kubernetes provider、正式应用证明签发、真实公网/NAT 双主机、设备加入 UI、完整 Web、加密同步和恢复/根轮换仍待实现。
