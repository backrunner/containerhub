# Stacks — interaction revision, v8.3

## Layout

Use a narrow project rail and a shared service table. Both panes fill the same available height; each scrolls independently. The detail heading, three compact counters and table headings remain fixed.

- Rail: 208 pt, or 164 pt below 900 pt. Every entry is 56 pt high with two lines, a 4 pt line gap and 2 pt between entries. The first line aligns the icon/name and trailing Compose/Swarm badge. The second aligns service count left and attention/running count right, without punctuation separators. Long names truncate without wrapping; the runtime badge keeps its width.
- Service rows: 68 pt, aligned selection/disclosure/name/status/ports/actions columns. Name and image truncate.
- Row actions: 32 × 32 pt, 8 pt gaps. Stop uses red. Destructive removal lives in the scoped confirmation menu.
- Counters: services, running containers (Compose) or replicas (Swarm), attention count. Running never means healthy.
- Light and dark share geometry; compact windows retain all lifecycle and replica controls.

## Expanded container overview

Expanding a service shows its corresponding **containers**, not scheduler tasks. Compose and Swarm use one component. Remove task history, current/desired state columns and task-ID controls from this section.

- Show container name/ID, optional node ID, observed state and health, CPU, memory usage / limit, and container ports.
- At 1120 pt and above use compact horizontal rows under one shared column heading. Below it, place labeled resource metrics on a second line. ID and node are separate inline fields. Keep published Swarm service endpoints in the section heading when the parent port column is hidden; do not present service routing-mesh ports as container host bindings.
- Two secondary 24 × 24 pt bordered controls, gap 8: copy full container ID and open the existing inspector.
- Four containers per page; show paging only when needed. Clamp the page after an inventory refresh. Keep page state scoped by environment, stack kind/name and service.
- Swarm uses `Status.ContainerStatus.ContainerID`, validated as a full 64-character hex ID. Exclude historical shutdown/remove tasks, tasks without a container ID and duplicate container IDs.
- Inspection is joined only by exact container identity and environment/provider. A remote or otherwise uninspected container shows its actual ID/node with “未采集”, “详情不可用” and unavailable metrics. Disable inspector access for that row; ID copy remains available. Never infer container health or state from scheduler state.
- An empty overview says “暂无可查看的容器”. Truncated inventory is explicitly marked. Old agents without container IDs remain compatible and do not produce fabricated entries.
- CPU/memory require fresh samples. Unsupported/stale/missing samples show “—”; a known memory limit remains distinct. The current Docker adapter does not yet implement live container stats; numeric review samples are isolated fixtures.

## Interaction and acceptance

Search project/service/image; filter Compose/Swarm or attention state. Select-all applies to visible services and retains other checked services. The heading toolbar targets checked services, or the whole project if none are checked. Row and context-menu actions always target the clicked service.

Replica editing and batch confirmation capture service versions/resource identities and revalidate before enqueueing. This revision manages existing deployments, not Compose YAML deployment or Swarm cluster provisioning.

Cover real-ID association, legacy-agent decoding, remote/unavailable inspection, healthy versus running, historical/pending exclusion, container paging and refresh clamping, inspector navigation, stale commands, scoped selection, same-name projects, read-only controls and six native size/theme cases. Prototype fixtures never connect to a host runtime. GPUI/Metal renders use isolated data and queues; real Docker/Swarm cluster acceptance is separate.

Provider schema reference: [Moby Swarm TaskStatus and ContainerStatus](https://github.com/moby/moby/blob/master/api/types/swarm/task.go).
