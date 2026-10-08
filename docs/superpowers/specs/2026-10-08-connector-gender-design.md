# TestStationView — Connector Gender

Date: 2026-10-08
Status: Accepted (requested by the user; the details below are design decisions taken while
implementing)

This extends the port types design (`2026-10-08-port-types-design.md`). It answers the "gender
(male/female)" item that `docs/future-work.md` lists under connector types.

## 1. Rules

- **Values:** a port has a **gender**:
  - Unspecified (the default);
  - Male;
  - Female;
  - Other (genderless or hermaphroditic connectors).
- **Independence:** gender is independent of the connector type. It is set like the type: in
  the New port form, for new ports, and in the port's Properties panel, as one undo step.
- **No cable rule:** a cable's own plugs have the opposite gender to the ports they go into,
  so any two port genders can be joined. For example, two male ports take a female–female
  cable. Gender therefore never warns or rejects. The type mismatch warning is unchanged.
- **Catalogue:** entries store each port's gender, and placing a model copies it.

## 2. Display

- **3D:** markers keep their type's shape and colour, and the gender changes their depth.
  - Male markers stick out 1.6× as far (like pins).
  - Female markers stick out 0.5× as far (nearly flush, like sockets).
  - Unspecified and Other markers are unchanged.
  - Cables plug into the front face, as before.
- **Properties:** a "Gender" dropdown under "Type".
- **Tree:** a port's detail shows its type and gender, as in "BNC male · Cable1". Unspecified
  parts are left out.

## 3. File format (version 5)

- `FORMAT_VERSION` becomes 5.
- Ports and catalogue ports gain `"gender"`, with one of the values `unspecified`, `male`,
  `female` or `other`.
- The field defaults to `unspecified` when it is missing, so migration 4 → 5 does nothing.
  The bump stops older apps from silently dropping genders.

## 4. Code shape

- **`tsv-core`:**
  - `Gender`, with `ALL`, `label` and `key`.
  - `Port::gender` and `ModelPort::gender`.
  - `PortSource::New { gender }`.
  - `plan_set_port_gender`.
  - Format v5.
- **`tsv-render`:**
  - `port_marker_box` takes the gender.
  - `layout::marker_depth`.
- **`tsv-app`:**
  - `DragSource::NewPort { gender }`.
  - `Session::set_port_gender`.
  - UI dropdowns and the tree detail.
