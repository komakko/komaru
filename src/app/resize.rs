use eframe::egui::{Context, CursorIcon, Pos2, Rect, ResizeDirection, ViewportCommand};

const GRAB_BORDER: f32 = 6.0;

fn is_maximized(ctx: &Context) -> bool {
    ctx.input(|input| input.viewport().maximized.unwrap_or(false))
}

pub fn grab_inset(ctx: &Context) -> f32 {
    if is_maximized(ctx) { 0.0 } else { GRAB_BORDER }
}

pub fn handle_edge_resize(ctx: &Context) {
    if is_maximized(ctx) {
        return;
    }
    let Some(position) = ctx.pointer_hover_pos() else {
        return;
    };
    let Some(direction) = edge_at(position, ctx.content_rect()) else {
        return;
    };
    ctx.set_cursor_icon(cursor_for(direction));
    if ctx.input(|input| input.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
    }
}

fn edge_at(position: Pos2, bounds: Rect) -> Option<ResizeDirection> {
    let north = position.y - bounds.top() < GRAB_BORDER;
    let south = bounds.bottom() - position.y < GRAB_BORDER;
    let west = position.x - bounds.left() < GRAB_BORDER;
    let east = bounds.right() - position.x < GRAB_BORDER;
    match (north, south, east, west) {
        (true, _, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::SouthEast),
        (_, true, _, true) => Some(ResizeDirection::SouthWest),
        (true, ..) => Some(ResizeDirection::North),
        (_, true, ..) => Some(ResizeDirection::South),
        (_, _, true, _) => Some(ResizeDirection::East),
        (_, _, _, true) => Some(ResizeDirection::West),
        _ => None,
    }
}

fn cursor_for(direction: ResizeDirection) -> CursorIcon {
    match direction {
        ResizeDirection::North => CursorIcon::ResizeNorth,
        ResizeDirection::South => CursorIcon::ResizeSouth,
        ResizeDirection::East => CursorIcon::ResizeEast,
        ResizeDirection::West => CursorIcon::ResizeWest,
        ResizeDirection::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeDirection::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeDirection::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeDirection::SouthWest => CursorIcon::ResizeSouthWest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(400.0, 300.0));

    fn at(x: f32, y: f32) -> Option<ResizeDirection> {
        edge_at(Pos2::new(x, y), BOUNDS)
    }

    #[test]
    fn every_edge_and_corner_is_grabbable() {
        assert_eq!(at(200.0, 1.0), Some(ResizeDirection::North));
        assert_eq!(at(200.0, 299.0), Some(ResizeDirection::South));
        assert_eq!(at(1.0, 150.0), Some(ResizeDirection::West));
        assert_eq!(at(399.0, 150.0), Some(ResizeDirection::East));
        assert_eq!(at(399.0, 1.0), Some(ResizeDirection::NorthEast));
        assert_eq!(at(1.0, 1.0), Some(ResizeDirection::NorthWest));
        assert_eq!(at(399.0, 299.0), Some(ResizeDirection::SouthEast));
        assert_eq!(at(1.0, 299.0), Some(ResizeDirection::SouthWest));
        assert_eq!(at(200.0, 150.0), None);
    }
}
