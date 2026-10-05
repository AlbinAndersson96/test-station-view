# Known issues

Minor findings from the final code review of each implementation plan that were deliberately
deferred. Major findings from those reviews were fixed before merging. None of these is
reachable with the default `Limits`, or they are cosmetic. Each entry names where the fix
belongs.

## Web app (`crates/app`, from PR #3)

- **Double re-render after every change.** `save_now` (`ui/core.rs`) sets the `rev` signal after
  every autosave even when nothing changed, so the tree, properties panel and toolbar render
  twice per edit. Only set it when `ui_revision` differs from the signal's current value.
- **The tree scrolls on every edit.** The scroll-into-view effect in `ui/tree.rs` runs on every
  revision, not only when the selection changes, so editing pulls the sidebar back to the
  selected row. Remember the previous selection and scroll only when it changes.
- **No-op edits create empty undo steps.** Committing an unchanged inline rename, renaming the
  document to its current name, or dropping a rack row onto itself still commits a plan. Undo
  then appears to do nothing. Skip commits whose document equals the current one.
- **Device loss is detected late.** `is_device_lost` is only checked inside `frame()`, so the
  overlay appears only after the next interaction. Have the lost callback request a frame.
- **`devicePixelRatio` changes without a resize are not handled.** Moving the window to a
  monitor with a different scale keeps the old canvas resolution. Observe
  `device-pixel-content-box` or a `matchMedia` resolution query.
- **The first camera is framed for a placeholder viewport.** The session starts with an
  800 × 600 viewport, and the first real resize does not reframe. On an unusually shaped window,
  part of the rack can be clipped until **Reset view**. Snap to the front view on the first
  `set_viewport`.
- **Stale port hover.** Hover is not cleared when the pointer leaves the canvas (no
  `pointerleave`), so a hovered port's label stays visible. The drag handles also forward
  pointer moves while nothing is being dragged.
- **The save banner blocks the canvas.** `.banner` needs `pointer-events: none`.
- **A field's blur could read a disposed signal.** `Field`'s blur handler uses
  `get_untracked`. Nothing triggers this today; `try_get_untracked` would make it safe.
- **Shortcuts work behind modal dialogs.** Ctrl+Z and Ctrl+Y still act while a dialog is open.
- **The context menu can open off-screen.** It is not clamped to the viewport near the right
  and bottom edges.

## Renderer (`crates/render`, from PR #2)

- **A NaN ray picks the first rack.** With a zero-size viewport, `pixel_to_ndc` yields NaN, and
  `hit_aabb` treats NaN as a hit. Return `None` when the ray is not finite.
- **`port_drop_target` panics if `Limits::port_cols` or `port_rows_per_u` is 0.** The
  subtraction overflows. Use `saturating_sub`, as `layout.rs` already does.
- **Labels can z-fight with their face when zoomed far out.** The 1 mm offset falls below depth
  precision near the maximum distance. Use reversed-Z or a depth bias on the label pipeline.
- **`Renderer::new_offscreen(0, 0)` panics.** Only tests use it. Clamp to 1 × 1 as `resize`
  does.
- **Dark fringes on white label text.** Labels use straight alpha with linear filtering.
  Premultiply on upload and use premultiplied blending.
- **Per-frame GPU churn.** Every frame creates its instance buffers anew and switches a bind
  group per label (about 500 at most). This is fine at the spec's scale. Persistent growable
  buffers and a label atlas would remove it.
- **Missing API conveniences.** The renderer does not expose the aspect ratio it uses, and
  device loss can only be polled, not subscribed to.
- **Test gaps.** The label cache has no tests. The GPU tests pass without checking anything on
  a machine with no GPU adapter.

## Core (`crates/core`, from plan 1)

- **`auto_rename` can exceed the name length limit** when `name_max_len` is shorter than the
  suffix (for example a limit of 2 and `_10`).
- **`validate` does not re-check name length against `Limits`.** Import checks it while
  parsing, but a document validated with a lower limit can pass.
- **Moving a device within its rack reorders the `devices` list.** The order in an exported
  file therefore changes even when nothing moved. All views sort by position, so this only
  adds diff noise.
- **`push_direction` maps NaN offsets to "push down"** without any signal.
- **A non-integer `format_version`** (for example `"1"` or `1.5`) reports "The file has no
  format version" instead of "malformed".
- **The port row count is computed inline** in `edit/port.rs` and `edit/device.rs` instead of
  through `Device::port_rows`.
