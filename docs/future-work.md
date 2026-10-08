# Future work

The MVP is a planning and documentation tool. The longer-term direction, set during the design
discussion, is a **digital twin of real test stations**: a shared, accurate record of what is
installed where and how it is connected, possibly showing live data from the instruments.

This page lists what was deliberately left out of the MVP and, where the code already prepares
for it, the extension point to build on. Nothing here has been designed in detail. Each item
needs its own design (spec, then plan) before implementation.

## Directions already prepared for in the code

### Equipment catalogue

Users would pick a device **type** (for example "Keysight 34465A DMM, 2U") from a catalogue
instead of entering the height by hand. Each placed instance keeps its own name.

- **Extension point:** `DeviceKind` in `crates/core/src/model.rs` currently has only `AdHoc`.
  A catalogue would add a variant that refers to a catalogue entry (for example by ID).
- **File format:** the `kind` field is already written to JSON and defaults to `ad_hoc` when it
  is missing, so new variants can be added with a format-version bump and a migration step (see
  [File format changes](#file-format-changes)).
- **Open questions:**
  - Is the catalogue built in, editable by the team, or stored with each document or
    separately?
  - Does a type also define default ports, colour or a front-panel image?

### Connector types for ports

Ports would carry a connector type (BNC, SMA, USB, LAN/RJ45, GPIB, banana, D-sub, …), possibly
with a different look per type.

- **Extension point:** `PortKind` in `crates/core/src/model.rs` currently has only
  `Unspecified`. The `kind` field is already in the file format, defaulting to `unspecified`.
- **Rendering:** port markers are drawn in `tsv-render`'s `scene::build_scene` and
  `layout::port_marker_box`. Per-type shapes or colours would start there.

### Cables between ports

**Done.** See `docs/superpowers/specs/2026-10-08-cables-design.md`. A cable joins two ports,
anywhere in the document, at most one per port. It has a name and a colour, and it is drawn in
3D as a sagging tube. Shift-dragging from a free port draws a cable; from a connected port it
re-plugs that cable's end.

- **Still open:**
  - cable type and length;
  - connector compatibility (once ports have a `PortKind`);
  - routing through cable trays, and rear-face ports;
  - several cables per port (splitters).
- **Extension points:**
  - `Cable` in `crates/core/src/model.rs` and its file-format twin `FileCable` (new fields need
    a format bump);
  - `layout::cable_path` for the shape;
  - the per-port rule in `plan_connect`, `plan_replug` and `validate::check_cables`.

### Shared backend storage

A server so the whole team works on the same documents instead of each browser's local
storage.

- **Extension point:** `DocumentStore` in `crates/app/src/storage.rs`. It is async on purpose,
  so an HTTP-backed store can replace `LocalStorageStore` without changing callers. It stores
  the document's JSON text. Startup recovery (`interpret_stored`) and autosave (`autosave`)
  already go through it.
- **Things the MVP does not handle yet:**
  - **More than one document:** local storage holds exactly one (`STORAGE_KEY`).
  - **Concurrent editing:** the MVP's rule is "last write wins", even between two tabs.
  - **Authentication.**
- **Also relevant:** IDs are UUIDs, so documents created in different browsers can be merged
  without ID clashes.

### Live data from instruments

Showing measurement status or values on the devices in 3D. Nothing is prepared for this
specifically. It would build on the backend, and probably on the catalogue (to know what each
instrument can report).

## Deliberately out of scope for the MVP

From the design spec (§2). These need design work before they can be added.

| Item | Notes / where it would touch |
|---|---|
| Half-width, fractional-height, configurable-depth or rear-mounted equipment | Placement in `core/src/placement.rs` is one-dimensional over whole U, and `Device` has no width, depth or side. This is the largest change: placement becomes 2D per side. |
| Rear-face ports | Ports are front-only (`Port` has no face). It needs a face field, a rear grid, and a rear camera view for port placement. |
| Free placement or rotation of racks, gaps between racks | Racks are laid out in a fixed row by `render/src/layout.rs::rack_left_x` (a fixed 40 mm gap). It needs rack positions in the model. |
| Multi-select | Selection is a single `Option<ObjectId>` in `Session`. |
| Several documents in local storage | See [Shared backend storage](#shared-backend-storage). |
| Cross-tab synchronisation | It could listen to the browser's `storage` event and reload or merge. |
| WebGL2 fallback | wgpu is built with WebGPU only. It needs wgpu's `webgl` feature and checks of shader and feature limits. |
| Automated visual tests of the 3D output | The renderer's offscreen pixel tests (`render/tests/gpu.rs`) are the starting point. |
| Hosting, CI pipeline, deployment automation | `trunk build --release` produces a static site in `crates/app/dist/`. Any static HTTPS host works. |

## Preliminary values meant to be tuned

These were marked "leave room for change" during the design and live in `Limits`
(`crates/core/src/limits.rs`), so changing them is a one-line edit plus tests:

- `name_max_len` = 10 characters (rack, device and port names).
- `port_rows_per_u` = 1 and `port_cols` = 5 (the port grid per U).
- `max_rack_height_u` = 100.

## Testing

Spec §7 planned a few `wasm-bindgen-test` tests in a headless browser. None were written,
because no browser could be installed in the development environment. Instead, all app logic
is tested natively, and the browser glue is covered by the manual checklist in plan 3, Task 6.
If a browser and driver become available (for example in CI), headless tests for
`LocalStorageStore` and the import path would close that gap.

## File format changes

Any model change that affects saved files needs these three steps:

- Bump `FORMAT_VERSION` in `crates/core/src/file_format.rs`.
- Add a step to `migrate` that upgrades the previous version's JSON.
- Keep the on-disk `File*` types separate from the model.

Files from a newer version are already rejected with "This file was made by a newer version of
the app."
