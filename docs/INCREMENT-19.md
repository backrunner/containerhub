# Increment 19 — Environment forms and Docker Stacks

Date: 2026-10-23. Local development implementation; not a release or live-cluster acceptance.

## Delivered

- Native environment add modal: existing Docker context, optional absolute CLI path, display name; or import an already-bound Agent controller profile. The latter preserves the existing binding and does not implement first-pair enrollment UI.
- Environment edit modal: name, notes, sampling interval, local/Docker CLI override; remote direct/relay/automatic connection policy and endpoints. Docker context identity is immutable; add another environment to change it. Context removal only removes the local catalog entry.
- Container creation screenshots cover the existing basic, advanced and startup sheets. Container editing currently means the Apple port-rebuild modal, with a danger “重建” action and explicit writable-layer loss. Full container configuration editing is not implemented. Docker port rebuild is disabled by capability.
- Shared typed Docker CLI provider, selected through `Runtime::docker`, desktop, Agent or Node. Every call uses the explicit context, no shell, bounded output/time, redacted errors and immutable container/service IDs.
- Compose projects: discover canonical project/service labels, inspect member containers and their independent health, start/stop/restart existing members, remove stopped members. One-off jobs are not restarted. This is not Compose `up/down`; YAML creation/update/deployment and dependency-order orchestration are not implemented.
- Swarm stacks: manager-only service/task inventory, modes, published ports, node IDs, desired/current task state, running/desired replicas and update status. Worker/unavailable states are explicit. Replicated services can be scaled to 0–10000, including zero to stop. Global/job modes cannot be scaled. Rolling restart applies only to replicated/global services. Removal deletes the captured services and their tasks, retaining networks, volumes, configs and secrets; it is not full `docker stack rm` cleanup.
- Compose and Swarm may share a name; their selection/API identities include the kind. Existing per-resource queues, persistent operation history and remote authorization are reused. Swarm changes require both container-management and lifecycle permission.
- Prototype v7.1 mirrors the Stack screens, add/edit forms and action spacing; environment forms use the same compact field order and fixed-size footer actions as native.

## Structure and safeguards

Core `stacks.rs` plans Compose/Swarm actions; `swarm.rs` owns service/task contracts. Docker inventory, parsing, process execution and mutations are separate modules, including dedicated Swarm read/write modules. Native Stack list, Compose details, confirmations, Swarm details and replica form are separate files. Node consumes Rust inventory/plans instead of duplicating label parsing.

Docker context Engine ID is pinned for the provider lifetime. Before each service mutation the adapter rechecks manager status, environment, immutable ID, stack membership and observed service version. A context change requires reopening/readding the environment. Version checking is a preflight check through Docker CLI, not an atomic compare-and-swap guarantee against concurrent external clients. After writing, the adapter confirms the service specification or removal. Success does **not** claim that a rollout has converged or every replica is healthy. Ambiguous writes remain UnknownOutcome and are not automatically replayed.

Only canonical Compose/Swarm membership labels enter the shared inventory; environment values, arbitrary label values, task error strings and Compose file paths are excluded. Swarm task containers are not targets for container lifecycle operations. Task details are bounded to 256 per service, with a truncation flag; native shows the latest 30, and truncated counts are marked as lower bounds. Stack batches are limited to 32 targets in native.

## Validation and evidence

- Workspace Rust tests, all-target/all-feature Clippy, Node Docker/Compose/Swarm operations and existing offline diagnostics regression.
- Isolated executable Docker fixture covers redaction, immutable identity, stale versions, replaced services, manager/worker gates, Global-mode restrictions, scale/restart/remove, remote environment remapping and resource preservation. It never contacts Docker.
- Real GPUI/Metal offscreen captures and interactions in light/dark at 760×560, 1040×700, 1440×900: equal-height columns, container form flows, environment add validation, cancellation, Stack confirmation, stale-member rejection, replica-number validation and stale-service rejection.
- Browser inspection of Compose/Swarm prototype, environment add tabs/edit forms, replica modal and same-name Stack selection.
- Logs: `.local/stacks-swarm-tests.log`, `.local/stacks-clippy.log`, `.local/stacks-swarm-ui.log`, `.local/stacks-node.log`, `.local/stacks-node-diagnostics.log`.
- Native screenshots: `.local/ui-validation/{theme}-{size}-{environment-add-docker,environment-add-agent,environment-edit,environment-edit-docker,create,create-advanced,create-startup,ports-modal,stacks,swarm,swarm-scale}.png`.
- Browser screenshots: `.local/visual-review/swarm-prototype.jpg`, `swarm-scale-prototype.jpg`, `environment-add-prototype.jpg`, `environment-edit-prototype.jpg`.

This host has the Docker CLI but no reachable Engine socket/Docker Desktop installation. Real Compose/Swarm cluster execution, remote Docker Agent over WAN, and Docker host telemetry are **not** validated. Docker currently supports container/resource inventory, safe basic creation, lifecycle, deletion and container inspection; logs, stats, image mutations, advanced creation and port rebuilding remain disabled. Apple runtime permissions and user containers were not changed.

## Usage

In native: 环境管理 → 添加环境 → Docker; enter an existing context, then switch to it. Stacks appears for Docker inventory.

Agent: `containerhub-agent serve ... --docker-context CONTEXT [--docker-binary /absolute/docker]` selects the Docker runtime while keeping the existing authenticated transport/binding model.

Node: `ContainerHub.connect({binary,stateDirectory,mode:'docker',dockerContext:'desktop-linux'})`. `stacks()` returns typed Compose/Swarm groups. `stackCommands(environment,name,action,kind)` captures plans; kind defaults to `compose`. Submit returned commands through the normal operation service. See the SDK README for scale and permission details.

Primary references: [Compose service labels](https://docs.docker.com/reference/compose-file/services/), [Swarm service update and rolling restart](https://docs.docker.com/reference/cli/docker/service/update/), [service scaling and Global restrictions](https://docs.docker.com/reference/cli/docker/service/scale/).

The fixture models Docker's 12-character service-list IDs. Inventory resolves them with `service inspect` before publishing full immutable keys; writes require the full 25-character ID. Removal verification queries by full ID rather than inferring absence from Stack labels. Reference: [Docker CLI service formatter](https://github.com/docker/cli/blob/master/cli/command/service/formatter.go).

Development bundles regenerated: `dist/ContainerHub.app` and `dist/node`. Application ad-hoc signature verified; Mach-O UUID and all file-backed sections match the current desktop build. The signed file's whole-file hash intentionally differs from the linker output. Bridge and SDK files match byte-for-byte. Hashes/section identities: `.local/stacks-artifacts.json`. No install, Git commit, publish or release signing was performed.
