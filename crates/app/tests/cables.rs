mod common;

use common::*;
use glam::Vec2;
use tsv_app::interaction::{Button, DragSource};
use tsv_app::session::Session;
use tsv_core::edit::ObjectId;
use tsv_core::ids::PortId;
use tsv_core::model::{Document, Rgb};
use tsv_render::layout::{CABLE_SEGMENTS, cable_points, port_anchor};

/// R1: DMM (U20) with HI, LO, SENSE; PSU (U18) with OUT. The ports are spread out so each is
/// well clear of the others on screen.
fn station() -> (Document, [PortId; 4]) {
    let dmm = with_ports(
        device("DMM", 20, 1),
        vec![port("HI", 0, 0), port("LO", 0, 2), port("SENSE", 0, 4)],
    );
    let psu = with_ports(device("PSU", 18, 1), vec![port("OUT", 0, 3)]);
    let ids = [
        dmm.ports[0].id,
        dmm.ports[1].id,
        dmm.ports[2].id,
        psu.ports[0].id,
    ];
    (doc(vec![rack("R1", 42, vec![dmm, psu])]), ids)
}

fn port_px(s: &Session, p: PortId) -> Vec2 {
    px(s, port_anchor(s.document(), &s.limits, p).unwrap())
}

/// Shift-drags from port `from` to canvas position `to` and releases there.
fn connect_drag(s: &mut Session, from: PortId, to: Vec2) {
    let start = port_px(s, from);
    s.pointer_down_with_shift(start, Button::Left, true);
    s.pointer_move(start + Vec2::new(5.0, 0.0), false, 0.0);
    s.pointer_move(to, false, 0.0);
    s.pointer_up(to, false, 0.0);
}

#[test]
fn shift_dragging_from_port_to_port_connects_them() {
    let (d, [hi, _, _, out]) = station();
    let mut s = session(d);
    let to = port_px(&s, out);
    connect_drag(&mut s, hi, to);
    let cables = &s.document().cables;
    assert_eq!(cables.len(), 1);
    assert_eq!((cables[0].a, cables[0].b), (hi, out));
    assert_eq!(s.selection(), Some(ObjectId::Cable(cables[0].id)));
    // One undo step; the selection goes with the cable.
    s.undo();
    assert!(s.document().cables.is_empty());
    assert_eq!(s.selection(), None);
    s.redo();
    assert_eq!(s.document().cables.len(), 1);
}

#[test]
fn a_plain_drag_still_moves_the_port() {
    let (d, [hi, _, _, out]) = station();
    let mut s = session(d);
    let (from, to) = (port_px(&s, hi), port_px(&s, out));
    s.pointer_down(from, Button::Left);
    s.pointer_move(from + Vec2::new(5.0, 0.0), false, 0.0);
    assert!(s.is_dragging(), "a port move shows the trash zone");
    s.pointer_up(to, false, 0.0);
    assert!(s.document().cables.is_empty());
}

#[test]
fn the_preview_follows_the_pointer_and_snaps_to_a_target_port() {
    let (d, [hi, _, sense, out]) = station();
    let mut s = session(d);
    let camera = *s.camera();
    let start = port_px(&s, hi);
    s.pointer_down_with_shift(start, Button::Left, true);
    s.pointer_move(start + Vec2::new(5.0, 0.0), false, 0.0);
    assert!(!s.is_dragging(), "no trash zone for cables");

    // Over empty space below the racks: a loose preview, nothing planned.
    s.pointer_move(Vec2::new(400.0, 590.0), false, 0.0);
    let preview = s.cable_preview().expect("loose preview");
    assert!(preview.valid);
    assert_eq!(
        preview.from,
        port_anchor(s.document(), &s.limits, hi).unwrap()
    );
    assert!(s.shown_document().cables.is_empty());
    assert_eq!(s.scene().tubes.len(), CABLE_SEGMENTS);

    // Over a free port: the planned cable is shown, and the port's name.
    s.pointer_move(port_px(&s, out), false, 0.0);
    assert_eq!(s.cable_preview(), None);
    assert_eq!(s.shown_document().cables.len(), 1);
    assert_eq!(s.hovered_port(), Some(out));
    assert_eq!(s.scene().tubes.len(), CABLE_SEGMENTS);
    assert!(s.document().cables.is_empty(), "nothing committed yet");

    // Over the start port itself: a neutral loose preview.
    s.pointer_move(port_px(&s, hi), false, 0.0);
    assert!(s.cable_preview().is_some_and(|p| p.valid));
    assert!(s.shown_document().cables.is_empty());

    s.pointer_move(port_px(&s, sense), false, 0.0);
    s.escape(0.0);
    assert!(s.document().cables.is_empty());
    assert_eq!(s.cable_preview(), None);
    assert_eq!(s.scene().tubes.len(), 0);
    assert_eq!(*s.camera(), camera, "the camera never moved");
}

#[test]
fn a_port_with_a_cable_cannot_take_another() {
    let (mut d, [hi, lo, _, out]) = station();
    d.cables = vec![cable("C1", hi, out)];
    let mut s = session(d);
    let start = port_px(&s, lo);
    s.pointer_down_with_shift(start, Button::Left, true);
    s.pointer_move(start + Vec2::new(5.0, 0.0), false, 0.0);
    s.pointer_move(port_px(&s, out), false, 0.0);
    let preview = s.cable_preview().expect("red preview");
    assert!(!preview.valid);
    assert_eq!(
        preview.to,
        port_anchor(s.document(), &s.limits, out).unwrap()
    );
    s.pointer_up(port_px(&s, out), false, 0.0);
    assert_eq!(s.document().cables.len(), 1);
    assert_eq!(s.revision(), 0, "nothing changed");
}

#[test]
fn releasing_elsewhere_changes_nothing() {
    let (d, [hi, ..]) = station();
    let mut s = session(d);
    connect_drag(&mut s, hi, Vec2::new(400.0, 590.0));
    assert!(s.document().cables.is_empty());
    assert_eq!(s.revision(), 0);
    assert!(!s.is_busy());
}

#[test]
fn the_port_panel_handle_starts_the_same_drag() {
    let (d, [_, lo, sense, _]) = station();
    let mut s = session(d);
    s.start_drag(DragSource::Cable { from: lo }, 0.0);
    let to = port_px(&s, sense);
    s.pointer_move(to, false, 0.0);
    s.pointer_up(to, false, 0.0);
    let c = &s.document().cables[0];
    assert_eq!((c.a, c.b), (lo, sense));
}

#[test]
fn cables_can_be_clicked_renamed_recoloured_and_deleted() {
    let (mut d, [hi, _, _, out]) = station();
    d.cables = vec![cable("C1", hi, out)];
    let id = d.cables[0].id;
    let mut s = session(d);
    let path = cable_points(s.document(), &s.limits, &s.document().cables[0]).unwrap();
    let middle = px(&s, path[CABLE_SEGMENTS / 2]);
    s.pointer_down(middle, Button::Left);
    s.pointer_up(middle, false, 0.0);
    assert_eq!(s.selection(), Some(ObjectId::Cable(id)));

    assert_eq!(s.rename(ObjectId::Cable(id), "Sense +"), Ok(()));
    assert_eq!(s.document().cables[0].name.as_str(), "Sense+");
    let red = Rgb { r: 255, g: 0, b: 0 };
    s.set_cable_color(id, red);
    assert_eq!(s.document().cables[0].color, red);

    s.delete(ObjectId::Cable(id));
    assert!(s.document().cables.is_empty());
    assert_eq!(s.selection(), None);
    assert_eq!(s.document().racks[0].devices[0].ports.len(), 3);
}

#[test]
fn deleting_a_port_takes_its_cable_and_undo_brings_both_back() {
    let (mut d, [hi, _, _, out]) = station();
    d.cables = vec![cable("C1", hi, out)];
    let mut s = session(d);
    s.delete(ObjectId::Port(out));
    assert!(s.document().cables.is_empty());
    s.undo();
    assert_eq!(s.document().cables.len(), 1);
    assert!(s.document().port(out).is_some());
}

#[test]
fn cables_are_exported_and_imported() {
    let (mut d, [hi, _, _, out]) = station();
    d.cables = vec![cable("C1", hi, out)];
    let s = session(d.clone());
    let (_, json) = s.export();
    let mut fresh = session(Document::new_default(&limits()));
    fresh.import(&json, 0.0).unwrap();
    assert_eq!(fresh.document().cables, d.cables);
}

#[test]
fn framing_a_cable_fits_its_path() {
    let (mut d, [hi, _, _, out]) = station();
    d.cables = vec![cable("C1", hi, out)];
    let id = ObjectId::Cable(d.cables[0].id);
    let mut s = session(d);
    s.frame(id, 0.0);
    let mut now = 0.0;
    settle(&mut s, &mut now);
    let path = cable_points(s.document(), &s.limits, &s.document().cables[0]).unwrap();
    for p in path {
        let q = px(&s, p);
        assert!(
            s.viewport().cmpge(q).all() && q.cmpge(Vec2::ZERO).all(),
            "{q}"
        );
    }
}
