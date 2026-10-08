# TestStationView — Port Types

Date: 2026-10-08
Status: Accepted (decisions taken with the user in the design discussion)

This extends the MVP design and the cables design (`2026-10-08-cables-design.md`). It answers
`docs/future-work.md` § "Connector types for ports".

## 1. Decisions

| Question | Decision |
|---|---|
| Which types? | A fixed built-in list (below). Adding one is a code change. |
| Look in 3D | Each type has its own marker shape and colour. |
| Effect on cables | Warn only. A cable between two different specific types is allowed, because adapters exist, but it is flagged. |
| Where is it set? | In the New port form (the type of the new port) and in the port's Properties panel. Both are undoable. |

## 2. Types

| Type | Label | File key | Marker shape | Marker colour |
|---|---|---|---|---|
| Unspecified | Unspecified | `unspecified` | square (as before) | the device's ink (as before) |
| BNC | BNC | `bnc` | round, 1.0 | `#d4a017` (gold) |
| SMA | SMA | `sma` | round, 0.6 | `#e07b20` (orange) |
| N-type | N-type | `n_type` | round, 1.2 | `#8a5cd6` (purple) |
| Banana | Banana | `banana` | round, 0.5 | `#d93636` (red) |
| USB | USB | `usb` | rectangle 1.2 × 0.5 | `#2a7de1` (blue) |
| LAN | LAN (RJ45) | `lan` | rectangle 1.0 × 0.8 | `#2bb673` (green) |
| GPIB | GPIB | `gpib` | rectangle 1.9 × 0.5 | `#1f3a93` (dark blue) |
| D-sub | D-sub | `d_sub` | rectangle 1.6 × 0.6 | `#6b7c8f` (slate) |
| Power | Power (IEC) | `power` | rectangle 1.2 × 0.9 | `#2b2b2b` (black) |
| Other | Other | `other` | square (as before) | the device's ink (as before) |

- **Sizes** are relative to today's square marker side, which is `0.45 × min(cell width, cell
  height)`.
- **Round markers** are short cylinders that stick out of the face, with that diameter.
- **Rectangles** are width × height.
- Every marker stays centred where today's marker is and sticks out `PORT_PROTRUSION_MM`.
- Cables plug into the centre of the marker's front face, as before.

## 3. Rules

- A port has exactly one type. New ports get the type chosen in the New port form, which
  defaults to Unspecified.
- Changing a port's type never changes anything else, even when its cable then mismatches.
- **Mismatch:** a cable mismatches when both its ports have a *specific* type (anything but
  Unspecified and Other) and the two types differ.
- A mismatch is a warning, never a rejection: connect and re-plug behave as before.

## 4. File format (version 3)

`FORMAT_VERSION` becomes 3. The format itself does not change shape: each port's `kind` takes
one of the file keys above. Migration 2 → 3 does nothing. The bump makes older apps report
"made by a newer version" instead of "malformed" when they meet a new type.

## 5. Interaction and display

- **New port form:** a "Type" dropdown, defaulting to Unspecified. The dragged ghost has the
  chosen type's shape.
- **Port properties:** a "Type" dropdown. A change is one undo step.
- **Cable drag** (connect or re-plug): over a port that would make a mismatched cable, the
  preview runs to that port in amber (`#f0a020`). Releasing still connects.
- **Cable properties:** a mismatched cable shows "⚠ BNC to SMA" (its ends' types).
- **Tree:**
  - a port's detail shows its type (unless Unspecified), then its cable's name;
  - a mismatched cable's detail starts with "⚠".

## 6. Code shape

- **`tsv-core`:**
  - `PortKind` gets its variants, `ALL`, `label()`, `key()`, `from_key()` and `is_specific()`.
  - `Document::cable_mismatch(&Cable) -> Option<(PortKind, PortKind)>`.
  - `PortSource::New` gets a `kind`.
  - `plan_set_port_kind`.
  - The file format adds the new keys, and version 3.
- **`tsv-render`:**
  - `layout::port_marker_box` takes the port's kind.
  - `scene` draws round markers as tubes along +Z and colours markers by type.
- **`tsv-app`:**
  - `DragSource::NewPort` gets a `kind`.
  - `Session::set_port_kind`.
  - Amber previews for mismatches.
  - UI: dropdowns, warnings and tree details.
