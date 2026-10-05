//! Vertical placement of devices in one rack (spec §3.4).
//! All arithmetic is in i64 so imported values cannot overflow.

use std::cmp::Reverse;
use std::collections::HashMap;

use crate::ids::DeviceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Occupant {
    pub id: DeviceId,
    pub bottom_u: u32,
    pub height_u: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementError {
    TooTall,
    NoRoomAbove,
    NoRoomBelow,
}

/// Clamps a snapped bottom U so a device of `height_u` lies inside the rack.
pub fn clamp_bottom(
    bottom_u: u32,
    height_u: u32,
    rack_height_u: u32,
) -> Result<u32, PlacementError> {
    if height_u > rack_height_u {
        return Err(PlacementError::TooTall);
    }
    Ok(bottom_u.clamp(1, rack_height_u - height_u + 1))
}

/// New bottoms for `others` after a device is dropped at `[bottom_u, bottom_u + height_u - 1]`.
/// Devices whose centre is above the dropped device's centre are pushed up, the rest down,
/// cascading. Output is in the same order as `others`.
pub fn push_for_drop(
    others: &[Occupant],
    bottom_u: u32,
    height_u: u32,
    rack_height_u: u32,
) -> Result<Vec<(DeviceId, u32)>, PlacementError> {
    let dropped_centre = 2 * i64::from(bottom_u) + i64::from(height_u) - 1;
    let (up, down): (Vec<Occupant>, Vec<Occupant>) = others
        .iter()
        .copied()
        .partition(|o| doubled_centre(o) > dropped_centre);
    let mut moved = HashMap::new();
    push_up(
        up,
        i64::from(bottom_u) + i64::from(height_u),
        rack_height_u,
        &mut moved,
    )?;
    push_down(down, i64::from(bottom_u) - 1, &mut moved)?;
    Ok(collect(others, &moved))
}

/// New bottoms for `others` after the device at `bottom_u` grows to `new_height_u` (upward).
pub fn push_for_growth(
    others: &[Occupant],
    bottom_u: u32,
    new_height_u: u32,
    rack_height_u: u32,
) -> Result<Vec<(DeviceId, u32)>, PlacementError> {
    let above: Vec<Occupant> = others
        .iter()
        .copied()
        .filter(|o| o.bottom_u > bottom_u)
        .collect();
    let mut moved = HashMap::new();
    push_up(
        above,
        i64::from(bottom_u) + i64::from(new_height_u),
        rack_height_u,
        &mut moved,
    )?;
    Ok(collect(others, &moved))
}

/// Twice the centre, so that half-U centres stay integral.
fn doubled_centre(o: &Occupant) -> i64 {
    2 * i64::from(o.bottom_u) + i64::from(o.height_u) - 1
}

/// `cursor` is the lowest U the group may occupy.
fn push_up(
    mut group: Vec<Occupant>,
    mut cursor: i64,
    rack_height_u: u32,
    moved: &mut HashMap<DeviceId, u32>,
) -> Result<(), PlacementError> {
    group.sort_by_key(|o| o.bottom_u);
    for o in group {
        let bottom = i64::from(o.bottom_u).max(cursor);
        let top = bottom + i64::from(o.height_u) - 1;
        if top > i64::from(rack_height_u) {
            return Err(PlacementError::NoRoomAbove);
        }
        moved.insert(o.id, bottom as u32);
        cursor = top + 1;
    }
    Ok(())
}

/// `cursor` is the highest U the group may occupy.
fn push_down(
    mut group: Vec<Occupant>,
    mut cursor: i64,
    moved: &mut HashMap<DeviceId, u32>,
) -> Result<(), PlacementError> {
    group.sort_by_key(|o| Reverse(i64::from(o.bottom_u) + i64::from(o.height_u)));
    for o in group {
        let height = i64::from(o.height_u);
        let top = (i64::from(o.bottom_u) + height - 1).min(cursor);
        let bottom = top - height + 1;
        if bottom < 1 {
            return Err(PlacementError::NoRoomBelow);
        }
        moved.insert(o.id, bottom as u32);
        cursor = bottom - 1;
    }
    Ok(())
}

fn collect(others: &[Occupant], moved: &HashMap<DeviceId, u32>) -> Vec<(DeviceId, u32)> {
    others
        .iter()
        .map(|o| (o.id, moved.get(&o.id).copied().unwrap_or(o.bottom_u)))
        .collect()
}
