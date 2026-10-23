# @containerhub/node

Node 22+ 的 ESM/TypeScript SDK，通过 `containerhub-bridge` 子进程复用 Rust 管理模块。支持 Apple container、Docker context、显式 Demo、已绑定远程 Agent；不依赖 GPUI。当前是本地开发包，尚未发布 npm。

## 构建与使用

在仓库根目录执行：

```sh
cargo build -p containerhub-bridge --locked
node tests/node-management.mjs  # 使用临时目录和示例数据，不操作本机容器
node tests/node-sdk-failures.mjs # 异常退出、断线 operation ID 与关闭超时
node tests/node-diagnostics.mjs # 隔离 CLI、离线诊断与参数能力门禁
node tests/node-runtime-service.mjs # 隔离 CLI、服务启动、持久化核对与重启不重放
```

以下示例从仓库根目录运行。改成 `mode: 'apple'` 会连接当前用户的真实 Apple container，资源操作的环境 ID 从快照读取；离线服务启动使用 diagnostics 返回的环境 ID。

```js
import { ContainerHub } from './packages/node/index.mjs';
import { resolve } from 'node:path';

const hub = await ContainerHub.connect({
  binary: resolve('target/debug/containerhub-bridge'),
  stateDirectory: resolve('.local/node-example'),
  mode: 'demo',
});
try {
  const snapshot = await hub.snapshot();
  console.log(snapshot.containers);
  const operation = await hub.execute({
    kind: 'pull_image',
    environment: snapshot.environment,
    reference: 'docker.io/library/nginx:alpine',
  });
  if (operation.state !== 'Succeeded') {
    throw new Error(`${operation.state}: ${operation.message}`);
  }
  console.log(operation.result);
} finally {
  await hub.close({ timeoutMs: 15 * 60_000 });
}
```

`execute()` 等待终态并返回 Operation；Failed/Cancelled/UnknownOutcome 也是正常的任务结果，调用方必须检查 state。只有协议、输入或连接错误会抛 ContainerHubError。`wait()` 的 AbortSignal 只停止等待，不取消运行时操作。

## API

完整类型见 [index.d.ts](index.d.ts)。所有方法返回 Promise。

| 方法 | 行为 |
| --- | --- |
| `diagnostics()` | CLI 路径/版本、服务连通状态、逐项能力与不可用原因；服务离线仍可返回 |
| `hostSnapshot()` | 目标主机 CPU、物理内存、系统盘容量/I/O、非回环网卡速率；首个差分样本为 null；独立于容器服务状态 |
| `snapshot()` | 环境、能力、全部容器和创建身份 |
| `resources('Image' \| 'Volume' \| 'Network')` | 资源列表 |
| `inspectContainer(key)` | 脱敏容器配置，前后核对原资源身份 |
| `reconcile(id)` | 持久化核对意图并查询原任务；不会重新提交命令，随后可 wait |
| `cancel(id)` | 持久化拉取取消请求；需继续 wait 查询最终结果 |
| `inspectImage(environment, reference)` | 脱敏镜像详情、digest、所有容器引用和其他标签 |
| `logs(key, 'Standard' \| 'Boot')` | 有界日志快照 |
| `exportLogs(key, { source, query })` | 新日志快照的有界筛选导出内容，不在 Agent 上写文件 |
| `stats(key)` | CPU/内存采样；CPU 首次为 null，不代表 0 |
| `submit(command, { operationId })` | 持久化受理后返回，ID 可由调用方预先保存 |
| `operation(id)` / `operations()` | 按原 ID 查询 / 最近 100 项 |
| `wait(id, { timeoutMs, signal })` | 等待终态，不重复提交 |
| `execute(command, options)` | submit + wait |
| `close({ timeoutMs })` | 关闭输入并等待已受理任务，默认 15 秒 |

Command 包含生命周期、镜像拉取、容器创建/删除、镜像删除、端口重建，以及本机 `start_runtime`。创建返回新容器的完整 key；默认停止，可选自动启动，失败后的部分结果也保留 key；容器删除只接受停止容器。不要自己构造或缓存过期 incarnation；同名新容器是另一个资源。

```js
const created = await hub.execute({ kind: 'create_container', environment, spec: {
  name: 'example', image: 'docker.io/library/nginx:alpine',
  cpus: 2, memory_mib: 512,
  ports: [{ host: '127.0.0.1', host_port: 8080, container_port: 80, protocol: 'tcp' }],
}});
if (created.state === 'Succeeded' && created.result?.kind === 'container') {
  const started = await hub.execute({ kind: 'lifecycle', key: created.result.value.key, action: 'Start' });
  console.log(started.state);
}
```

创建前必须先拉取镜像。SDK 不自动执行额外命令，也不在网络重试时重新创建资源。

## 镜像详情与日志文件

```js
const details = await hub.inspectImage(environment, 'docker.io/library/nginx:alpine');
console.log(details.variants, details.used_by, details.other_references);
// 用户确认后可提交：{ kind: 'delete_image', key: details.key }
// 执行前重新核对 digest；有运行或停止容器引用时拒绝删除。

const exported = await hub.exportLogs(containerKey, { source: 'Standard', query: 'error' });
// 内容有资源身份、时间、筛选条件和有限尾部提示，不是完整日志历史。
const { writeFile } = await import('node:fs/promises');
await writeFile(exported.file_name, exported.content, { flag: 'wx', mode: 0o600 });
```

镜像 `key` 与容器 ResourceKey 不同：它包含 environment、provider、reference、digest。删除只针对该引用，保留其他标签；CLI 的孤立内容清理失败会报告 UnknownOutcome。`inspectImage` 只返回环境变量名称，不包含值、命令参数或标签值。

`exportLogs` 使用现有日志权限，所有文件写入留给本机调用方。桌面导出冻结当前已显示的快照，而 SDK 此方法会先取得一次新快照。两者使用同一格式与过滤规则。日志本身可能含应用输出的敏感信息，导出内容不进入任务历史或自动同步。

## 远程与凭据

```js
const hub = await ContainerHub.connect({
  binary: '/absolute/path/containerhub-bridge',
  stateDirectory: '/absolute/path/node-state',
  mode: 'remote',
  profile: '/absolute/path/controller-profile',
  // agent: 'p256:…', // profile 有多个 Agent 时显式选择
});
```

先按 [远程开发说明](../../docs/REMOTE-DEVELOPMENT.md) 建立绑定和授权。profile 内已有直连/中继地址及固定的 Agent 身份；Node 不接受任意未验证 Agent 连接，也不获取 owner/device 私钥。同一 profile 有实例锁，切换客户端前需关闭原客户端。

旧 Native profile 没有新增管理权限时，由所有者显式运行 `containerhub-agent enable-management --profile …`；Web 设备默认只读/日志，新增管理权限须单独签发。正式应用证明与云端同步仍待实现。

## 断线和持久化

需要可靠恢复的调用先生成并持久化正的 JS 安全整数 operationId，再调用 submit。相同 ID/相同命令返回原任务；相同 ID/不同命令报 Conflict。同一个 stateDirectory 的历史不能随意删除，否则会失去去重证据。

超时/断开错误携带 operationId（若已知）。重连后先 `operation(operationId)`；返回 UnknownOutcome 时调用 `reconcile(operationId)` 查询原任务；仍未知时核对实际资源。不要换一个 ID 自动重做写操作。旧日志没有自动重放，去重记录上限 100000 条，满后停止接受新任务。

SDK 单次 RPC 等待 15 秒；任务 wait 默认 15 分钟。`close()` 默认等待 15 秒，耗时拉取应先 wait 或调长关闭期限；异常退出或超时强制关闭均返回 UnknownOutcome。强制关闭只结束客户端进程，不能保证运行时写入已经停止。桥接与 Agent 仍保存两层日志；新远程任务在发送前持久化 `receipt`，包含 Agent/所有者 ID、原请求 ID 和原期限。重启后最多自动排入 32 项核对，按原关联查询，不重新 Submit；旧记录没有关联时不能补造。Agent 自身中途重启或记录缺失仍保持 UnknownOutcome。

结构与传输约束见 [ARCHITECTURE.md](../../docs/ARCHITECTURE.md)。`scripts/package-node.py` 将 SDK 和当前平台的普通 bridge 构建打包到 `dist/node`，不上传或安装任何包。


## 创建选项、进度与取消

```js
const task = await hub.submit({ kind: 'pull_image', environment, reference: 'docker.io/library/nginx:alpine' });
// 取消按钮调用 await hub.cancel(task.id)，保留原 ID；重复请求幂等。
const result = await hub.wait(task.id, { onProgress: operation => console.log(operation.progress) });
// Cancelled = 确认未执行；UnknownOutcome = 后台结果需核对；已确认成功仍是 Succeeded。
const created = await hub.execute({ kind: 'create_container', environment, spec: {
  name: 'web', image: 'docker.io/library/nginx:alpine', cpus: 1, memory_mib: 256,
  options: { networks: ['default'], user: '1000:1000', working_directory: '/app', read_only: true }
} });
if (created.state === 'Succeeded') console.log(await hub.inspectContainer(created.result.value.key));
```

`options.volumes` 可挂载已有命名卷，例如 `[{name:'database-data',destination:'/data',read_only:true}]`。秘密环境变量与启动参数见下文专用输入，宿主目录挂载未开放。上例用户/只读配置须符合所选镜像运行要求。

`progress` 为 waiting/fetching/unpacking/verifying/creating/starting，可能跳过很快的阶段；不是百分比。`onProgress` 收到每次查询到的完整任务快照。`AbortSignal` 只停止等待；`cancel(id)` 才发送取消请求。取消只支持拉取，运行中可能得到 UnknownOutcome，已下载内容不会回滚。详情没有环境/标签值或启动参数正文。

## CLI 路径与能力诊断

Apple 模式可传 `appleBinary: '/absolute/path/container'`，仅接受本机绝对路径；未传时按已知安装位置发现。此配置不通过远程 RPC 修改 Agent 路径，Agent 主机由启动参数 `serve --apple-binary …` 配置。显式路径失效会报告错误，不回退到其他 CLI。

```js
const report = await hub.diagnostics();
console.log(report.cli_path, report.cli_version, report.service_error);
for (const check of report.checks) console.log(check.feature, check.available, check.reason);
```

能力描述 CLI 命令/参数契约，与服务可连接是两个维度。写入仍只开放已适配的 1.6.0；未知版本保留通过帮助检查的读取，未验证写入关闭。`capabilities.create_advanced` 单独表示高级创建选项。远程报告按当前设备授权过滤；能力结果不能替代每次请求的权限校验。

容器 `label_names` 仅含标签名称，不含值；可与名称/镜像一起搜索。排序使用 core 的确定性规则，桌面持久化排序偏好，SDK 调用方可按所需展示顺序处理返回列表。

## 原任务恢复

```js
const original = await hub.operation(operationId);
if (original.state === 'UnknownOutcome') {
  await hub.reconcile(original.id);
  const checked = await hub.wait(original.id);
  console.log(checked.state, checked.receipt);
}
```

`reconcile` 对执行中/核对中和已有明确终态的任务返回原记录。查询失败、权限失效、Agent/所有者改变或响应与原命令不符时保留 UnknownOutcome。可在恢复网络/权限后再次核对；写请求过期不阻止读取历史结果。关联中只有公开标识，没有私钥。直连与中继可以查询同一个绑定中的原请求。

恢复中的取消针对原 Agent 请求；不会把本地尚在排队的“核对任务”当成尚未执行的远程写任务。以前已持久化但未确认的取消意图可幂等重送，Submit 不会重送。自动恢复/人工核对与新任务共享 4 项并发和同资源串行限制。旧本机 Start/Stop 可按相同 incarnation 的目标状态核对，其余未知结果不从资源有无推断成功。

## 一次性秘密环境变量

```js
const snapshot = await hub.snapshot();
if (!snapshot.capabilities.create_secrets) throw new Error('当前运行时不支持秘密注入');
const task = await hub.execute({
  kind: 'create_container', environment: snapshot.environment,
  spec: { name: 'private-app', image: 'example.invalid/app:v1', cpus: 2, memory_mib: 512,
    options: { secret_environment: ['API_TOKEN'] } },
}, { secrets: { API_TOKEN: tokenFromYourSecretStore } });
```

先拉取镜像；`tokenFromYourSecretStore` 由调用方取得，不要硬编码在普通配置或源码。公共 command 只含名称，`secrets` 是独立的一次性输入。支持空值，最多 32 项，每值 16 KiB、总计 64 KiB，不支持空字符或换行；声明与输入必须完全一致。

`submit`/`execute` 接受相同的 `{ operationId, secrets }` 选项。携值重交已用 ID 会返回 Conflict，即使值相同；应先 `operation(id)`/`reconcile(id)` 查原结果。排队输入随进程退出丢失，需重新填写并明确使用新 ID；已发送的 Agent 请求只查原记录。历史、进度回调和 inspect 不返回这些值。

Apple 使用 stdin 管道，不把值写入 argv 或临时文件；远程值在绑定后的加密通道传输。容器和运行时仍可读取它们，也可能自行记录日志。SDK 不提供持久化秘密库，不能保证清除调用方的 JavaScript 字符串或系统内存副本。

## 启动程序与参数

```js
const snapshot = await hub.snapshot();
if (!snapshot.capabilities.create_startup) throw new Error('当前运行时不支持启动命令覆盖');
const created = await hub.execute({
  kind: 'create_container', environment: snapshot.environment,
  spec: { name: 'worker', image: 'example.invalid/worker:v1', cpus: 2, memory_mib: 512,
    options: { startup: { entrypoint: '/app/worker', arguments: ['--queue', 'daily jobs', ''] } } },
});
```

先拉取镜像；创建默认保持停止，也可选择下文的创建后启动流程。省略 startup 保留镜像默认程序和参数；`startup: { arguments: [...] }` 保留镜像 ENTRYPOINT、替换 CMD；指定 entrypoint 后丢弃镜像默认 CMD，arguments 为空时仅执行指定程序。空 entrypoint/空覆盖对象会报 InvalidData；当前不支持清空镜像 ENTRYPOINT 的特殊模式。

最多 64 项参数，每项及启动程序最多 4096 字节，总计最多 32 KiB；禁止 NUL，启动程序不能是空白或含控制字符。数组顺序、空值、空格、引号和符号原样保留；不要传入需要 SDK 拆分的整条 shell 命令字符串。普通配置会进入任务记录，秘密值应通过 secrets 参数和名称声明传递。inspect 仍只返回参数数量。

同 ID 调整参数、顺序或启动程序会返回 Conflict；先查询原任务结果，再为明确的新操作使用新 ID。没有能力时 provider 拒绝覆盖，默认创建仍可用。

## 创建后立即启动

先检查 `snapshot.capabilities.create_and_start`，然后在 `spec.options` 设置 `start_after_create: true`。默认 false 不改变原有创建行为。公共队列和 Agent 在启动前持久化新容器的完整资源键，并在启动后查询运行状态；远程同时需要 ManageContainers 和 Lifecycle，创建期间吊销也会阻止启动。

```js
const task = await hub.execute({
  kind: 'create_container', environment: snapshot.environment,
  spec: { name: 'worker', image: 'example.invalid/worker:v1', cpus: 1, memory_mib: 256,
    options: { start_after_create: true } },
});
if (task.state === 'Failed' && task.result?.kind === 'container') {
  const key = task.result.value.key;
  // 用户修复启动原因并决定重试后，使用新任务只启动原容器，无需再次提交秘密值。
  // await hub.execute({ kind: 'lifecycle', key, action: 'Start' });
}
```

Failed/UnknownOutcome 中的 Container 表示创建已确认，不表示启动成功；返回的状态是当时观察值，操作前仍须刷新。不要重新提交 Create 或用名称查找新容器来替换该 key。UnknownOutcome 先使用 `reconcile(task.id)` 查询原任务；恢复不会重复创建或自动补启动。同名对象已被替换时旧 key 的 Start 会被拒绝。Agent 自身中断时保留 UnknownOutcome 和已知创建身份。

### 重建端口映射

`rebuild_ports` 使用当前资源的完整 key、expected_state、expected 旧端口和 ports 新端口。它是删除原容器后重建的破坏性操作；调用方须向用户说明可写层丢失、不保留原容器及失败不回滚。仅在 snapshot.capabilities.rebuild_ports 可用时提供入口。成功结果包含新的资源 key；后续命令必须使用新 key，不能重放旧 key。详见 [增量 16](../../docs/INCREMENT-16.md)。

## 启动本机运行时服务

服务离线时 `snapshot()` 会失败，先用独立 `diagnostics()` 获取环境 ID 和 `start_runtime` 能力。调用方应在用户明确选择启动后提交；不会自动提升权限或安装内核，也不会发送容器启动命令。

```js
const report = await hub.diagnostics();
if (!report.checks.some(c => c.feature === 'start_runtime' && c.available)) {
  throw new Error('当前环境不支持服务启动');
}
// operationId 由调用方预先生成并持久化；失败/未知状态也必须处理。
const task = await hub.execute({ kind: 'start_runtime', environment: report.environment }, { operationId });
if (task.state === 'UnknownOutcome') {
  await hub.reconcile(task.id); // 只核对服务状态，不重复启动
  console.log(await hub.wait(task.id));
}
```

Apple 1.6.0 使用 `system start --disable-kernel-install --timeout 30`，进程期限 40 秒；仅服务状态确认可连接才记为成功。服务已运行时不会再次调用启动。首次内核安装、macOS 授权或其他宿主配置仍由用户手动处理。该命令不可取消；SDK 默认关闭期限短于最大执行时间，需要先等待任务或显式延长 `close` 期限。

本轮仅支持本机与 Demo，远程 client 和 Agent 均拒绝；已有设备 Lifecycle 授权不包含主机服务管理。应用/bridge 重启后只检查中断任务，不自动重新执行。详见 [增量 17](../../docs/INCREMENT-17.md)。

## 主机遥测

`hostSnapshot()` 返回公共 `HostSnapshot`，包含目标环境 ID、UNIX 秒时间戳和 `sample` 示例标记。CPU 是整机归一到 100%；网络为非回环接口合计，可能含 VPN/虚拟接口；磁盘只读取系统可写文件系统，不重复累加 APFS 卷。首次 CPU/网络/I/O 差分缺失不代表零。采样建议间隔至少 2 秒。

远程遥测沿用已绑定 Agent 和设备的 Read 权限，通过现有加密通道。返回环境必须匹配固定 Agent 身份。`Container.health` 区分 healthy、unhealthy、starting、unconfigured、unsupported、unknown；停止容器不计入运行中健康比例。Apple adapter 当前为 unsupported。


## Docker / Compose / Swarm

```js
const docker = await ContainerHub.connect({
  binary: '/absolute/containerhub-bridge', stateDirectory: '/absolute/private-state',
  mode: 'docker', dockerContext: 'desktop-linux',
  // dockerBinary: '/opt/homebrew/bin/docker', // optional absolute override
});
const groups = await docker.stacks();
const swarm = groups.find(s => s.kind === 'swarm');
if (swarm) {
  const service = swarm.services.find(s => s.swarm?.mode === 'replicated')?.swarm;
  if (service) await docker.execute({ kind: 'swarm_service', key: service.key,
    stack: service.stack, version: service.version, action: { kind: 'scale', replicas: 2 } });
}
// Plans do not execute until submitted; inspect targets before submitting a batch.
const compose = groups.find(s => s.kind === 'compose');
if (compose) {
  const commands = await docker.stackCommands(compose.environment, compose.name, 'start', 'compose');
  for (const command of commands) await docker.execute(command);
}
await docker.close();
```

`stackCommands(environment,name,action,kind='compose')` uses Rust planning. Compose actions: `start`, `stop`, `restart`, `remove_stopped` (existing containers only, one-off jobs excluded from lifecycle). Swarm actions: `restart`, `stop` (all members must be replicated, scales to zero), `remove_services`. Start Swarm services through an explicit replica count; Global/job modes are not scaled. A Compose and Swarm stack may share a name, so always retain `kind`.

Swarm inventory requires a manager. `snapshot.swarm.status` distinguishes inactive, worker, manager and unavailable; service tasks include node IDs and a truncation flag. Mutations bind immutable service ID, observed version and stack membership. Native and Node task success confirms service specification changes, not rollout convergence or health. Unknown outcomes are not replayed. Agent operations require both ManageContainers and Lifecycle permissions and reuse the existing encrypted transport. A local Docker context uses the Docker CLI's own context credentials; it does not create an Agent binding.

Removal preserves networks, volumes, configs and secrets. YAML deployment/update, Compose up/down, Docker host sampling, logs/stats, image mutation, advanced creation and Docker port rebuild are not implemented. Docker context Engine ID pinning lasts for the provider lifetime; external changes between preflight inspection and CLI mutation remain a concurrency boundary. Full evidence and limitations: `docs/INCREMENT-19.md`.
