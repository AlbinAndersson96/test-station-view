//! Placement of ports on a device's front-face grid (spec §3.5).

use std::collections::HashMap;

use crate::ids::PortId;

/// Row 0 is the bottom row of the face; column 0 is the leftmost column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cell {
    pub row: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSize {
    pub rows: u32,
    pub cols: u32,
}

impl GridSize {
    pub fn contains(self, cell: Cell) -> bool {
        cell.row < self.rows && cell.col < self.cols
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushDir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortPlacementError {
    OutOfGrid,
    NoRoom,
}

/// `(dx, dy)` is the cursor's offset from the target cell's centre (+x right, +y up).
/// The occupant is pushed away from the side the cursor is on; the horizontal axis wins ties,
/// and the exact centre pushes right.
pub fn push_direction(dx: f32, dy: f32) -> PushDir {
    if dx.abs() >= dy.abs() {
        if dx > 0.0 { PushDir::Left } else { PushDir::Right }
    } else if dy < 0.0 {
        PushDir::Up
    } else {
        PushDir::Down
    }
}

/// New cells for `occupants` after a port is dropped on `target`. An occupant of `target`
/// moves one cell in `dir`, cascading through further occupied cells. Output is in input order.
pub fn push_for_drop(
    occupants: &[(PortId, Cell)],
    target: Cell,
    dir: PushDir,
    grid: GridSize,
) -> Result<Vec<(PortId, Cell)>, PortPlacementError> {
    if !grid.contains(target) {
        return Err(PortPlacementError::OutOfGrid);
    }
    let by_cell: HashMap<Cell, PortId> = occupants.iter().map(|(id, c)| (*c, *id)).collect();
    let mut shifted: HashMap<PortId, Cell> = HashMap::new();
    let mut cell = target;
    while let Some(&id) = by_cell.get(&cell) {
        let next = step(cell, dir, grid).ok_or(PortPlacementError::NoRoom)?;
        shifted.insert(id, next);
        cell = next;
    }
    Ok(occupants
        .iter()
        .map(|(id, c)| (*id, shifted.get(id).copied().unwrap_or(*c)))
        .collect())
}

fn step(cell: Cell, dir: PushDir, grid: GridSize) -> Option<Cell> {
    let next = match dir {
        PushDir::Up => Cell { row: cell.row.checked_add(1)?, ..cell },
        PushDir::Down => Cell { row: cell.row.checked_sub(1)?, ..cell },
        PushDir::Left => Cell { col: cell.col.checked_sub(1)?, ..cell },
        PushDir::Right => Cell { col: cell.col.checked_add(1)?, ..cell },
    };
    grid.contains(next).then_some(next)
}
