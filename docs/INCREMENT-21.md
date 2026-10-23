# Increment 21 — Stack container quick overview

The expanded service section now shows corresponding containers for both Compose and Swarm. Task history, current/desired task state and task-ID actions have been removed from this section. The project/service management controls retain their existing behavior.

## Implementation

- `core/stacks/overview.rs` supplies the shared actual-container projection. Docker captures the allowlisted `Status.ContainerStatus.ContainerID` field. Exact full ID and environment/provider joins expose local inspection; remote/uninspected containers retain their identity without invented status, health or metrics.
- Pending tasks without a container ID, shutdown/remove history, invalid IDs and duplicates do not become container rows. Legacy agent payloads may omit `container_id`. The Node type includes this optional field; its development package has been rebuilt.
- The native and v8.1 prototype overview includes identity/node, observed state/health, CPU, memory / smaller limit, ports, full-ID copy and inspector navigation. Rows share one component. Four containers per page, with refresh clamping and environment/project/service-scoped state.
- Wide windows use a horizontal row; narrow windows place resource metrics below identity. Narrow Swarm views retain service-level published ports separately from container bindings. Remote details are disabled while copying the actual ID remains available.
- Fresh container samples are consumed on the Stacks page when supported. The current Docker adapter does not implement live container stats; missing/stale samples are “—”, not zero or demo numbers. Numeric review data are explicitly isolated fixtures.
- Prototype inspector navigation now avoids rebinding Stack-only controls on another page; its health and environment footer follow the selected container/environment.

## Validation

- 104 workspace Rust tests passed, including exact-container association, duplicate/history/pending exclusion, cross-environment isolation, legacy-agent decoding and Docker CLI fixture parsing. Existing service command identity/version and secret redaction tests remain passing.
- Clippy passed for workspace/all targets/all features with `-D warnings`.
- GPUI/Metal offscreen rendering and interactions passed at 760×560, 1040×700 and 1440×900, in light and dark themes. Checks include inspection of the exact container, disabled unavailable inspection, page navigation/clamping, scoped service actions and stale service rejection. Actual rendered PNGs were visually reviewed.
- Edge prototype review covered light Swarm, dark Compose, 760×560 layout, container inspection and unavailable-container state. Narrow panes have equal 356 px height, no horizontal page/card overflow. The corrected v8.1.1 asset cache version produced no new console errors during inspector navigation. All prototype JavaScript syntax checks passed.
- `dist/ContainerHub.app` was rebuilt and its ad-hoc signature verified. `dist/node` was rebuilt; distributed and source TypeScript declarations match.

Evidence: `.local/stack-overview-{tests,ui,clippy,package,node-package}.log`, `.local/stack-overview-artifacts.json`.

Screenshots:

- [Native Swarm light](../.local/visual-review/stacks-v81-native-swarm-light.png)
- [Native Compose dark](../.local/visual-review/stacks-v81-native-compose-dark.png)
- [Native compact](../.local/visual-review/stacks-v81-native-760.png)
- [Prototype Swarm light](../.local/visual-review/stacks-v81-swarm-prototype.jpg)
- [Prototype Compose dark](../.local/visual-review/stacks-v81-compose-dark-prototype.jpg)

These are isolated fixture/prototype and real GPUI renderer checks, not a live Docker/Swarm cluster acceptance test. No user containers were changed. No installation, Git publication or release was performed.

## v8.2 follow-up — compact project rail

The project rail now uses two lines per entry: name and trailing Compose/Swarm badge, then service count and attention/running summary. Entry height is 56 pt, line gap 4 pt and inter-entry gap 2 pt. The summary aligns with the project name; long names truncate and badges retain their width. Native and HTML prototype use the same geometry.

Validation: the existing six GPUI/Metal size/theme and interaction suites passed; desktop all-target/all-feature Clippy passed. Edge verified two 18/14 px lines, 56 px entries, selection and no horizontal overflow at 760×560 and 1440×900. Native light compact and dark wide captures were visually inspected. The desktop development bundle was rebuilt. This is a layout-only follow-up; workspace domain tests were not rerun.

Evidence: `.local/stack-compact-rail-{ui,clippy,package}.log`; screenshots `.local/visual-review/stacks-v82-*`.
