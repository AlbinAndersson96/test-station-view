# TestStationView — Example Station on First Visit

Date: 2026-10-08
Status: Accepted (requested by the user; the details below are design decisions taken while
implementing)

## 1. Behaviour

- **First visit:** when nothing is stored in the browser (or storage is unavailable), the app
  opens the **example station** instead of the empty `Station1`.
  - It is not saved until the user changes something, so an untouched example is rebuilt (and
    picks up improvements) on every visit.
  - Once the user edits, their document is autosaved and restored as before.
- **"Example" toolbar button:** replaces the current document with the example after a
  confirmation ("You can undo this"), exactly like New.
- **Unchanged:**
  - **New** still creates the empty `Station1` document (MVP spec §3.1).
  - When stored data cannot be loaded, the fallback stays the empty document, so the recovery
    dialog is not confused with example content.

## 2. Contents

The example is built by `tsv_core::example::example_document` through the ordinary edit
plans, and a test checks that it obeys every rule and shows every feature:

- **Racks:** three, of different heights.
  - "Measure" (24U): LAN switch, signal generator, oscilloscope, DMM.
  - "Power" (18U): two power supplies and a PDU.
  - "DUT" (12U): a test fixture.
- **Catalogue:** five models.
  - The switch, generator, scope, DMM and PSU are catalogue models.
  - The PSU model is placed twice (PSU1 and PSU2).
  - The PDU and the fixture are ad hoc.
- **Ports:** every connector type and every gender appears. For example:
  - BNC, SMA and N-type on the fixture;
  - a male IEC mains inlet on the PDU;
  - a genderless "Other" port and an unspecified spare.
- **Cables:** eleven named, coloured cables, several of them between racks.
  - Power is red and black, LAN is green, and the DMM sense lead is yellow.
  - One deliberate BNC → SMA connection shows the ⚠ mismatch warning.
