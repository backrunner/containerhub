# Increment 22 — quieter container lists

The native app and v8.3 runnable prototype use tighter, aligned container lists with fewer repeated labels and no middle-dot-joined metadata in the revised surfaces. The work covers the main Containers table, the Stack project rail and expanded container overview.

## Changes

- Main rows are 56 pt with two identity lines. CPU and memory have separate right-aligned columns. Port pairs use an arrow and have additional separation from memory. The fixed action group remains 64/28/28 pt with 8 pt gaps and a red Stop control.
- Resource columns depend on the actual list width, including the inspector animation, rather than only window width. Below 800 pt the list prioritizes identity, state, ports and actions. Pagination and the inspector retain equal bottom edges.
- Stack entries retain their compact two-line structure. The second line aligns service count left and attention/running count right. Wide expanded overviews use shared column headings; narrow layouts retain per-field labels on the second row. Container ID and node are separate fields.
- Revised captions, confirmations and the inspector environment line no longer use decorative dot separators. Meaningful colored state indicators remain.
- The native container row is extracted into `apps/desktop/src/containers/row.rs`. Stopped rows no longer consume retained running samples; invalid CPU samples render as unavailable. Runtime/provider behavior is otherwise unchanged.

Design: [list specification](../.agents/LIST-REFINEMENT.md), [Stack specification](../.agents/STACK-REDESIGN.md).

## Validation

- Existing GPUI interaction and Metal rendering suites passed at 760×560, 1040×700 and 1440×900 in both light and dark themes. The final run includes the port-column spacing refinement. Selection, paging, sorting, inspector navigation/animation and scoped Stack operations remain covered.
- Desktop all-target/all-feature Clippy passed with `-D warnings`; formatting and all 17 prototype JavaScript syntax checks passed. No additional implementation-mirroring tests were added, and workspace domain tests were not rerun for this UI-only change.
- Edge review covered 760, 1040, 1280 and 1440 px widths, light/dark, open/closed inspector, main list and Swarm overview. Measured 56 px rows, 28 px row controls, 8 px action gaps, equal pane bottoms and no horizontal page overflow. At a 688 px list width resources hide while ports remain; at 816 px they are visible. Selecting five entries and paging retained exactly five selections. No browser warnings/errors were recorded.
- Visually reviewed the actual native renders and browser screenshots. Rebuilt `dist/ContainerHub.app` and verified its ad-hoc signature. Binary SHA-256 and artifact paths are in `.local/list-refinement-artifacts.json`.

Evidence: `.local/list-refinement-ui-final.log`, `.local/list-refinement-clippy-final.log`, `.local/list-refinement-package.log`.

- [Native container list](../.local/visual-review/lists-v83-native-light.png)
- [Native dark list](../.local/visual-review/lists-v83-native-dark.png)
- [Native Swarm overview](../.local/visual-review/lists-v83-native-swarm.png)
- [Native compact overview](../.local/visual-review/lists-v83-native-compact.png)
- [Prototype container list](../.local/visual-review/lists-v83-light-prototype.jpg)
- [Prototype compact list](../.local/visual-review/lists-v83-compact-prototype.jpg)
- [Prototype Swarm overview](../.local/visual-review/lists-v83-stacks-prototype.jpg)

Native images are actual GPUI/Metal offscreen renders using isolated fixtures. Prototype actions modify browser fixtures only. These checks do not establish live Docker/Swarm telemetry or cluster behavior; no user containers, installation, Git publication or release were changed.
