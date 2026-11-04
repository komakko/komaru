use eframe::egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Vec2};

use crate::core::rgba::Rgba;

const CELL: f32 = 6.0;
const DISC_CELL: f32 = 4.0;
const LIGHT: Color32 = Color32::from_gray(204);
const DARK: Color32 = Color32::from_gray(153);

pub fn color32(color: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

pub fn checker(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 0.0, LIGHT);
    let columns = (rect.width() / CELL).ceil() as usize;
    let rows = (rect.height() / CELL).ceil() as usize;
    let mut mesh = Mesh::default();
    for row in 0..rows {
        let first = 1 - row % 2;
        for column in (first..columns).step_by(2) {
            let min = rect.min + Vec2::new(column as f32, row as f32) * CELL;
            mesh.add_colored_rect(
                Rect::from_min_size(min, Vec2::splat(CELL)).intersect(rect),
                DARK,
            );
        }
    }
    painter.add(Shape::mesh(mesh));
}

pub fn checker_disc(painter: &Painter, center: Pos2, radius: f32) {
    painter.circle_filled(center, radius, LIGHT);
    let cells = (2.0 * radius / DISC_CELL).ceil() as usize;
    let origin = center - Vec2::splat(radius);
    let mut mesh = Mesh::default();
    for row in 0..cells {
        let first = 1 - row % 2;
        for column in (first..cells).step_by(2) {
            let min = origin + Vec2::new(column as f32, row as f32) * DISC_CELL;
            let cell = Rect::from_min_size(min, Vec2::splat(DISC_CELL));
            let inside = [
                cell.left_top(),
                cell.right_top(),
                cell.left_bottom(),
                cell.right_bottom(),
            ]
            .iter()
            .all(|corner| corner.distance(center) <= radius);
            if inside {
                mesh.add_colored_rect(cell, DARK);
            }
        }
    }
    painter.add(Shape::mesh(mesh));
}

pub fn gradient(
    mesh: &mut Mesh,
    rect: Rect,
    [top_left, top_right, bottom_left, bottom_right]: [Color32; 4],
) {
    let first = mesh.vertices.len() as u32;
    mesh.colored_vertex(rect.left_top(), top_left);
    mesh.colored_vertex(rect.right_top(), top_right);
    mesh.colored_vertex(rect.left_bottom(), bottom_left);
    mesh.colored_vertex(rect.right_bottom(), bottom_right);
    mesh.add_triangle(first, first + 1, first + 2);
    mesh.add_triangle(first + 1, first + 3, first + 2);
}
