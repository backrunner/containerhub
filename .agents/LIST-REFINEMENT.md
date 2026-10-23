# Container lists — v8.3

Keep the list useful for scanning and operating containers. Avoid decorative copy, nested cards and middle-dot-separated metadata. Use aligned fields and short labels; reserve colored status dots for actual state indicators.

## Main container table

- A 56 pt row contains two identity lines: container name (12 pt, medium) and image (11 pt, muted), separated by 3 pt. Long values truncate.
- Use separate, right-aligned CPU and memory columns. Show them when the **available list pane** is at least 800 pt wide, accounting for the sidebar, content padding and animated inspector width. Keep identity, state, port and actions visible at smaller widths.
- Render published port pairs as `3000 → 3000`. Separate the port column from memory with 16 pt of effective space.
- Keep actions visible and stable: 64 × 28 pt lifecycle control, then two 28 × 28 pt icon controls, with 8 pt gaps. Stop remains red. Buttons never reflow with name length or state.
- Selection, sorting, pagination and inspector behavior retain their existing scope. The table and inspector keep equal bottom edges; table content scrolls independently of pagination.
- Stopped containers do not display retained running samples. Missing/stale/invalid CPU samples render as “—”. UI refinements do not add Docker telemetry support.

## Stack container overview

- The project rail remains two lines and 56 pt high. Its second line separates service count on the left from attention/running count on the right. No punctuation joins these fields.
- Expanded services use one shared column heading above the container rows at wide sizes. Do not repeat CPU, memory and port labels in every row.
- Below 1120 pt, put labeled resource values on the second row so values retain their meaning without squeezing identity or actions.
- Container ID and node are separate inline fields with 12 pt spacing. Keep actual container state and health distinct; remote/uninspected entries remain explicitly unavailable.
- Memory limits retain the smaller `/ limit` presentation. Service published ports remain distinct from container bindings.

The runnable HTML prototype is `design/index.html` (asset version v8.3). Native implementation is in `desktop/src/containers.rs`, `containers/row.rs` and `stacks/instances.rs`. Visual and interaction evidence is recorded in `docs/INCREMENT-22.md`.
