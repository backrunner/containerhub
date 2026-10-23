# 远程控制开发增量 11

2026-10-22。实现了 Rust Agent、出站 Relay、Native 客户端和浏览器协议模块。当前验证使用隔离的 FakeProvider 与本机 TCP/WebSocket；尚未完成正式应用证明、真实公网/NAT、Safari/Chrome 或完整 Web 产品验收。

## 已实现的控制路径

- 第一个完成应用凭据验证、设备签名和 SQLite 绑定事务的所有者获得绑定；提前打开连接不会占有 Agent。绑定后其他所有者不能覆盖。
- Agent 固定所有者根公钥，Native 固定 Agent 公钥；Native 和 Web 使用各自设备密钥及根签名授权。授权文件只包含公钥、签名策略、地址及回执，不包含根私钥。
- 直连和出站中继共用绑定、权限、吊销与请求去重数据库。Relay 只转发端到端密文；路由身份另外用 Agent 签名校验。
- 按 Agent、环境和动作授权，支持只读、日志、生命周期、镜像管理、容器管理和信任管理。每个操作重新检查授权；在线会话约每秒检查吊销，未执行的排队任务也重新检查。快照能力按实际设备授权过滤。
- Native 根私钥只在 Rust 凭据层使用。短期策略临近到期时自动续期，保留吊销记录；新策略先持久化再发出。普通 Web 设备没有自行续期或签发其他设备的权力。
- 远程读列表/详情、镜像/卷/网络列表、镜像脱敏详情与删除、应用/启动日志、CPU/内存统计、启停/重启、拉取镜像、创建和删除停止容器。GPUI 和 Node bridge 使用相同远程 ManagementClient；日志导出在客户端格式化与保存，不给 Agent 发送文件路径。公共模块见 [ARCHITECTURE.md](ARCHITECTURE.md)。
- 写操作先持久化再 ACK，记录发起 owner/device、Agent、请求 ID、完整资源键和结果。队列最多 32 项、同时最多 4 项，同资源串行。丢失 ACK 后查询原请求 ID，不能自动生成新 ID 重做；Agent 重启后未完成操作标为 UnknownOutcome。

## 正式凭据与开发夹具

正式 Agent 初始化必须指定独立受信的应用凭据签发公钥。第一次认领同时需要这个签发者给控制设备签发的短期 `AppCredential`、所有者设备授权和本次会话的持钥证明。没有凭据则拒绝认领。

**App Attest 验证器、正式签名/entitlement 和应用凭据签发服务尚未实现。** `import-credential` 是对接入口，不会根据导入成功就声称凭据可信；签名由 Agent 校验。当前没有可用的生产发行信任根。不能把自签测试凭据当作“官方 ContainerHub”证明。

`development` feature 仅用于隔离测试：开发 Agent 强制 literal loopback 地址并固定 FakeProvider，开发控制端也只能连接 loopback。普通构建拒绝开发 profile/state。测试签发私钥随机生成，不内置、不复用，也不提供公开注册服务。

macOS 正式 Owner/Device/Agent 私钥存 Keychain，配置仅保存引用；开发夹具和非 macOS 后备实现使用 0700 目录 / 0600 文件。这里没有声称 Keychain 私钥为硬件不可导出。浏览器用 WebCrypto 生成不可导出的 P-256 私钥，保存在 IndexedDB；页面脚本仍能调用其签名能力，XSS 防护仍是 Web 产品要求。

## 本机复现

```sh
cargo build -p containerhub-agent -p containerhub-bridge --features development --locked
cargo build -p containerhub-remote --example development_fixture --features development --locked

# 路径必须是新的目录；只创建测试身份与示例数据
target/debug/examples/development_fixture .local/remote-demo
target/debug/containerhub-agent relay --listen 127.0.0.1:7444
# 在另一终端运行
target/debug/containerhub-agent serve --state .local/remote-demo/agent \
  --listen 127.0.0.1:7443 --relay ws://127.0.0.1:7444

# 在控制端终端认领一次；输出已验证的绑定回执
target/debug/containerhub-agent connect --profile .local/remote-demo/controller \
  --url ws://127.0.0.1:7443 --claim
target/debug/containerhub-agent snapshot --profile .local/remote-demo/controller
cargo run -p containerhub --features remote-development -- \
  --remote-profile .local/remote-demo/controller
```

中继连接用 `connect --url ws://127.0.0.1:7444 --relay-agent '<AgentId>' --agent '<AgentId>'`，仍带同一个 `--profile`。AgentId 是回执公钥的协议指纹，可从 `serve` 的诊断输出取得。`--agent` 必须已绑定；`--claim` 只用于显式首次认领，不能替代重新连接时的 pin。

同一个 profile 有实例锁；桌面或 Node bridge 打开时，先关闭再用 CLI 修改该 profile。当前 GPUI 可以从环境管理导入已绑定的远程 profile，并通过顶部环境入口切换独立会话；首次绑定与设备授权仍使用下述 CLI 流程。`--demo` 与 `--remote-profile` 不可混用。

新建 Native owner profile 包含镜像/容器管理权限。旧 profile 不静默扩权，需要所有者显式运行 `containerhub-agent enable-management --profile …` 后重新连接。该命令只更新本机 owner 设备的签名授权，不改 Web 设备授权。

生产流程的命令入口为 `identity`、`init --app-issuer`、`import-credential`、`connect`；在应用证明服务完成以前，只能用于集成验证。公网客户端要求有效证书的 `wss://`；Agent/Relay 服务端当前提供 WS listener，需要部署者配置 TLS 反向代理。没有部署公网 Relay、自动证书、系统服务安装器或自动端点发现。

## 浏览器设备共享已有绑定

1. 浏览器调用 `loadBrowserDevice()`，得到独立非导出设备私钥及可导出的公钥。
2. 通过可信方式把该公钥交给现有 Native 管理员。CLI 入口为 `grant --profile … --device-public browser-public.json --output browser-grant.json`；默认只读/日志，显式 `--allow-lifecycle` 允许启停，`--allow-management` 允许镜像拉取/删除与容器创建/删除，两项独立授权。
3. 把生成的公开授权包通过可信通道交回浏览器。该包包含现有 Agent pins，不能从不可信云端直接接受被替换的 pin。Web 调用 `connectRemote` 即可访问这些已有 Agent，无需重新认领每台主机。
4. `revoke --profile … --device '<DeviceId>'` 签发更高版本策略并推送已知 Agent；离线失败会逐台报告。策略默认 1 小时、上限 24 小时，离线设备到期后拒绝控制；设备授权上限 30 天，CLI Web 授权默认 24 小时。

这里实现的是跨端身份与传输模块；扫码/授权 UI、SvelteKit 页面、云端加密同步、浏览器授权自动续期、恢复备份和根轮换仍未实现。同步根公钥与签名授权即可共享控制关系；不必把根私钥复制到 Web。

## 协议 v1

实现入口：`crates/remote/src/{crypto,identity,channel,store,network,server,client}.rs`；浏览器封装：`packages/client-js/remote.mjs`。

| 项目 | 编码与约束 |
| --- | --- |
| 密钥交换 | snow 的 `Noise_NN_25519_ChaChaPoly_SHA256`；NN 本身不认证身份，随后必须完成双向 P-256 会话签名，才允许 RPC |
| Noise prologue | `ContainerHub.Remote.v1;auth=P256-SHA256;framing=1` |
| 身份公钥 | P-256 SEC1 uncompressed 65 字节，base64url 无 padding；指纹是 `p256:` + SHA-256(该 base64url 字符串 UTF-8) 的 base64url |
| 签名 | ECDSA P-256/SHA-256，P1363 `r || s` 64 字节；签原始 JSON payload 字节，不要求各端重新序列化后相同 |
| 签名输入 | `ContainerHub-Signature\0` + u32BE(domain 字节数) + domain UTF-8 + u32BE(payload 字节数) + payload |
| 域隔离 | `containerhub.` + `device-grant` / `owner-policy` / `app-credential` / `agent-proof` / `client-proof` / `relay-proof` / `binding-receipt` + `.v1` |
| 会话绑定 | AgentProof 和 ClientProof 签名包含本次 Noise handshake hash 与 Agent 公钥；不接受别的会话或主机的证明 |
| 帧/消息 | 二进制 WebSocket，单帧至多 65535 字节；加密前分片含 u32BE 总长度与 offset；单记录至多 1 MiB，严格顺序重组 |
| 授权 | 签名覆盖设备、范围、权限和期限；策略版本最大 JS 安全整数，不能减少吊销记录，同版异文拒绝 |

不能仅凭 TLS 或 Noise 握手成功认定已授权。首次没有预置 Agent pin 的认领仍是 TOFU；合法应用资格不能证明对方就是目标主机的主人，先来先绑定的首次抢占边界不变。

绑定 SQLite 使用 WAL/FULL、立即事务、独立安装标记与绑定检查点。丢数据库、损坏或单独回滚数据库会拒绝重新认领，需要恢复；具有宿主权限的攻击者回滚整套磁盘/密钥不在此保证内。操作去重表上限 100000 条，满时拒绝新操作，不静默删除去重证据；归档/维护流程待做。

## 构建与验证

需要 `wasm32-unknown-unknown` 标准库，以及与 Cargo.lock 匹配的 wasm-bindgen CLI 0.2.129。脚本可以显式选择带目标标准库的 rustc，不修改全局工具链：

```sh
python3 scripts/build-browser.py --rustc /path/to/rustc --wasm-bindgen /path/to/wasm-bindgen
# 先按上文编译 development Agent 与 fixture
node tests/remote-browser.mjs
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

构建产生 `.local/browser-protocol` 的 Node 测试模块和 `dist/browser-protocol` 的浏览器 ES modules。互操作脚本自动创建临时夹具与 loopback Agent/Relay，检查 WebCrypto 私钥不可导出、Native 首次认领、Web 同一所有者授权、直连/中继、只读拒绝写入及在线吊销，最后只清理自己的测试进程和文件。

本增量共 36 项 Rust 测试、6 组 GPUI 结构/交互检查，另有 Node 本机管理旅程、异常退出及远程/WebCrypto/WASM/WebSocket 互操作旅程。原始输出见 `.local/validation-increment-5.txt`。远程脚本覆盖镜像详情/删除/日志导出，以及只读 Web 对包括镜像删除在内的管理动作拒绝。Node 提供了真实密码 API 和网络连接，但不能替代 Safari/Chrome 的 IndexedDB、CSP、WSS、混合内容、本地网络访问权限及页面生命周期验收；也不能替代两台真实主机与公网/NAT 测试。


## 增量 6 管理 RPC

新增 `inspect_container { key }`（Read）和 `cancel { request_id }`（目标环境 ManageImages，仅限拉取）。取消按原所有者日志查询并先落盘，排队取消确认未执行；运行中的结果仍可能 UnknownOutcome。RemoteOperation 新增默认兼容字段 `progress`、`cancel_requested` 与 Cancelled 终态。客户端只重试原取消请求，不重新 Submit。

`create_container.spec.options` 贯通远程网络/命名卷/用户/工作目录/只读配置；检查 DTO 不含环境/标签值或启动参数正文。详细语义与 loopback 验证边界见 [INCREMENT-6.md](INCREMENT-6.md)。正式应用凭据签发、公网/NAT、云同步与完整 Web 仍未验收。

## 增量 7 诊断 RPC

`diagnostics` 是 Read 权限的只读请求，返回映射后的 Agent 环境、CLI 路径/版本、服务状态和逐项能力原因。管理/日志能力会按当前设备授权过滤，每次管理请求仍重新检查权限。服务离线仍保留已安装 CLI 信息。Node SDK 使用 `diagnostics()`，Web 协议可发送 `{ method: 'diagnostics' }`。

Agent 的本机运行时可以通过 `serve --apple-binary /absolute/path/container` 配置；该路径必须来自主机可信配置，不通过远程 RPC 接收。开发 FakeProvider 模式拒绝此参数。专项验证与剩余工作见 [INCREMENT-7.md](INCREMENT-7.md)。

## 增量 8 原任务恢复

桌面/Node 的本机日志持久化公开 receipt：Agent ID、owner ID、原 request ID 和期限；落盘后才 Submit。恢复使用既有 operation 查询并校验完整请求与绑定，权限/连接错误保留未知；到期不阻止历史查询。直连与出站中继可继续查询相同绑定。

Node 新增 `reconcile(id)`，桌面“核对原任务”使用同一公共队列。自动最多排入 32 项，执行并发 4；不重新 Submit。Agent 自身重启的未完成结果仍是 UnknownOutcome，已保存的取消意图只对原 ID 幂等发送。验证与边界见 [INCREMENT-8.md](INCREMENT-8.md)。

## 增量 9 一次性秘密输入

Agent 新增加密 RPC `submit_with_secrets { request: Mutation, secrets: { NAME: value } }`。Mutation/Command 仅声明 `options.secret_environment` 名称；值只在接收和执行内存中保留，ManageContainers 权限和环境在受理/执行两次核对。SQLite 不持久化值，也不存可用于离线猜测值的摘要。同 ID 携值重交一律冲突，客户端只查询原 request ID。既有增量 8 恢复无需秘密，也不会重新提交。能力诊断新增 `secret_environment`，快照为 `create_secrets`。详见 [增量 9](INCREMENT-9.md)。

## 增量 10 启动配置

创建命令增加可选 `options.startup`，沿用已有 Submit/SubmitWithSecrets、ManageContainers 校验及原请求关联。未设置时序列化不增加该字段；同 ID 调整参数顺序也视为异文。重启只查询原任务，保留原启动配置；公共参数入日志，秘密值仍不入日志。Agent 快照和诊断增加 `create_startup`/`startup_command` 并按环境授权过滤。直连/中继恢复、Node 远程创建及 Web 只读拒绝回归见 [增量 10](INCREMENT-10.md)。

## 增量 11 创建后启动

`options.start_after_create` 可选，缺省 false 且序列化省略；create_and_start 能力同时要求创建和生命周期权限。Agent 接收及执行前检查两项授权，创建完成后先将完整容器身份写入 SQLite，再复查授权后启动。期间吊销会保留创建结果且禁止启动。RemoteClient 把查询到的部分结果同步给本机检查点；失败不丢失容器身份。控制端重启只查询原 ID；Agent 重启保留检查点与 UnknownOutcome，不自动补执行。直连/中继控制端中断、权限/吊销与 Node 旅程证据见 [INCREMENT-11.md](INCREMENT-11.md)。


## Docker Agent runtime (increment 19)

Production `serve` accepts `--docker-context NAME` with optional `--docker-binary /absolute/path`; it conflicts with the Apple CLI override. The selected context must already exist for the Agent user. Development/demo serving rejects this override. Docker service inventory requires a Swarm manager; service changes need both ManageContainers and Lifecycle. Existing direct/relay transport, binding, request deduplication and permission checks remain in force. Docker-specific real host/WAN tests have not been performed; see [Increment 19](INCREMENT-19.md).
