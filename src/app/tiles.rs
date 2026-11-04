use std::ops::Range;
use std::sync::Arc;

use eframe::egui::epaint::Vertex;
use eframe::egui::{
    Color32, ColorImage, Context, ImageData, Mesh, Painter, Pos2, Rect, Shape, TextureHandle,
    TextureOptions, Vec2,
};

use crate::core::edit::Orientation;

pub const TILE_SIDE: usize = 2048;
const MARGIN: usize = 1;
const BYTES_PER_PIXEL: usize = 4;
const UPLOAD_BYTES_PER_FRAME: usize = 64 * 1024 * 1024;

struct Tile {
    texture: TextureHandle,
    uv: Rect,
}

pub struct TiledLevel {
    name: String,
    size: [usize; 2],
    columns: usize,
    rows: usize,
    options: TextureOptions,
    source: Option<Arc<ColorImage>>,
    tiles: Vec<Option<Tile>>,
}

impl TiledLevel {
    pub fn lazy(name: String, source: Arc<ColorImage>, options: TextureOptions) -> Self {
        let size = source.size;
        let columns = size[0].div_ceil(TILE_SIDE);
        let rows = size[1].div_ceil(TILE_SIDE);
        Self {
            name,
            size,
            columns,
            rows,
            options,
            source: Some(source),
            tiles: (0..columns * rows).map(|_| None).collect(),
        }
    }

    pub fn uploaded(
        ctx: &Context,
        name: String,
        source: Arc<ColorImage>,
        options: TextureOptions,
    ) -> Self {
        let mut level = Self::lazy(name, source, options);
        for index in 0..level.tiles.len() {
            level.upload(ctx, index);
        }
        level.source = None;
        level
    }

    pub fn set_options(&mut self, options: TextureOptions) {
        self.options = options;
        self.tiles.iter_mut().for_each(|tile| *tile = None);
    }

    pub fn size(&self) -> [usize; 2] {
        self.size
    }

    pub fn paint(
        &mut self,
        ctx: &Context,
        painter: &Painter,
        image_rect: Rect,
        orientation: Orientation,
        tint: Color32,
    ) -> bool {
        let visible = image_rect.intersect(painter.clip_rect());
        if visible.width() <= 0.0 || visible.height() <= 0.0 {
            return true;
        }
        let oriented_size = orientation.size(self.size);
        let view = View {
            image_rect,
            points_per_pixel: Vec2::new(
                image_rect.width() / oriented_size[0] as f32,
                image_rect.height() / oriented_size[1] as f32,
            ),
            source_size: self.size.map(|side| side as f32),
            orientation,
        };
        let (columns, rows) = self.visible_tiles(&view, visible, oriented_size);

        let mut budget = UPLOAD_BYTES_PER_FRAME;
        let mut complete = true;
        for row in rows {
            for column in columns.clone() {
                let index = row * self.columns + column;
                if self.tiles[index].is_none() && budget > 0 {
                    budget = budget.saturating_sub(self.upload(ctx, index));
                }
                let Some(tile) = &self.tiles[index] else {
                    complete = false;
                    continue;
                };
                painter.add(tile_mesh(
                    tile,
                    view.quad(self.tile_bounds(row, column)),
                    tint,
                ));
            }
        }
        complete
    }

    fn visible_tiles(
        &self,
        view: &View,
        visible: Rect,
        oriented_size: [usize; 2],
    ) -> (Range<usize>, Range<usize>) {
        let to_source = view.orientation.inverse();
        let oriented_size = oriented_size.map(|side| side as f32);
        let corner = |screen: Pos2| {
            let local = screen - view.image_rect.min;
            to_source.map(
                oriented_size,
                [
                    local.x / view.points_per_pixel.x,
                    local.y / view.points_per_pixel.y,
                ],
            )
        };
        let (first, last) = (corner(visible.min), corner(visible.max));
        (
            tile_range(first[0].min(last[0]), first[0].max(last[0]), self.columns),
            tile_range(first[1].min(last[1]), first[1].max(last[1]), self.rows),
        )
    }

    fn tile_bounds(&self, row: usize, column: usize) -> Rect {
        let min = Pos2::new((column * TILE_SIDE) as f32, (row * TILE_SIDE) as f32);
        let max = Pos2::new(
            ((column + 1) * TILE_SIDE).min(self.size[0]) as f32,
            ((row + 1) * TILE_SIDE).min(self.size[1]) as f32,
        );
        Rect::from_min_max(min, max)
    }

    fn upload(&mut self, ctx: &Context, index: usize) -> usize {
        let Some(source) = &self.source else {
            return 0;
        };
        let (row, column) = (index / self.columns, index % self.columns);
        let own = pixel_span(self.size, row, column);
        let padded = padded_span(self.size, &own);
        let bytes = padded.width() * padded.height() * BYTES_PER_PIXEL;
        let image: ImageData = if padded.covers(self.size) {
            Arc::clone(source).into()
        } else {
            crop(source, &padded).into()
        };
        let uv = own.uv_within(&padded);
        let texture = ctx.load_texture(format!("{}.{index}", self.name), image, self.options);
        self.tiles[index] = Some(Tile { texture, uv });
        bytes
    }
}

struct Span {
    columns: Range<usize>,
    rows: Range<usize>,
}

impl Span {
    fn width(&self) -> usize {
        self.columns.len()
    }

    fn height(&self) -> usize {
        self.rows.len()
    }

    fn covers(&self, size: [usize; 2]) -> bool {
        self.width() == size[0] && self.height() == size[1]
    }

    fn uv_within(&self, outer: &Span) -> Rect {
        let to_uv =
            |value: usize, start: usize, extent: usize| (value - start) as f32 / extent as f32;
        Rect::from_min_max(
            Pos2::new(
                to_uv(self.columns.start, outer.columns.start, outer.width()),
                to_uv(self.rows.start, outer.rows.start, outer.height()),
            ),
            Pos2::new(
                to_uv(self.columns.end, outer.columns.start, outer.width()),
                to_uv(self.rows.end, outer.rows.start, outer.height()),
            ),
        )
    }
}

fn pixel_span(size: [usize; 2], row: usize, column: usize) -> Span {
    Span {
        columns: column * TILE_SIDE..((column + 1) * TILE_SIDE).min(size[0]),
        rows: row * TILE_SIDE..((row + 1) * TILE_SIDE).min(size[1]),
    }
}

fn padded_span(size: [usize; 2], own: &Span) -> Span {
    Span {
        columns: own.columns.start.saturating_sub(MARGIN)..(own.columns.end + MARGIN).min(size[0]),
        rows: own.rows.start.saturating_sub(MARGIN)..(own.rows.end + MARGIN).min(size[1]),
    }
}

fn crop(source: &ColorImage, span: &Span) -> ColorImage {
    let stride = source.size[0];
    let mut pixels = Vec::with_capacity(span.width() * span.height());
    for row in span.rows.clone() {
        let start = row * stride;
        pixels.extend_from_slice(
            &source.pixels[start + span.columns.start..start + span.columns.end],
        );
    }
    ColorImage {
        size: [span.width(), span.height()],
        source_size: Vec2::new(span.width() as f32, span.height() as f32),
        pixels,
    }
}

fn tile_range(start: f32, end: f32, count: usize) -> Range<usize> {
    let first = ((start / TILE_SIDE as f32).floor().max(0.0) as usize).min(count);
    let last = ((end / TILE_SIDE as f32).ceil().max(0.0) as usize).min(count);
    first..last
}

struct View {
    image_rect: Rect,
    points_per_pixel: Vec2,
    source_size: [f32; 2],
    orientation: Orientation,
}

impl View {
    fn quad(&self, bounds: Rect) -> [Pos2; 4] {
        [
            bounds.left_top(),
            bounds.right_top(),
            bounds.right_bottom(),
            bounds.left_bottom(),
        ]
        .map(|corner| {
            let [x, y] = self.orientation.map(self.source_size, [corner.x, corner.y]);
            Pos2::new(
                self.image_rect.min.x + x * self.points_per_pixel.x,
                self.image_rect.min.y + y * self.points_per_pixel.y,
            )
        })
    }
}

fn tile_mesh(tile: &Tile, quad: [Pos2; 4], tint: Color32) -> Shape {
    let uvs = [
        tile.uv.left_top(),
        tile.uv.right_top(),
        tile.uv.right_bottom(),
        tile.uv.left_bottom(),
    ];
    let mut mesh = Mesh::with_texture(tile.texture.id());
    mesh.vertices
        .extend(quad.into_iter().zip(uvs).map(|(pos, uv)| Vertex {
            pos,
            uv,
            color: tint,
        }));
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    Shape::mesh(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::edit::Edit;

    fn gradient(width: usize, height: usize) -> ColorImage {
        let pixels = (0..width * height)
            .map(|index| {
                Color32::from_rgb((index % width % 251) as u8, (index / width % 251) as u8, 7)
            })
            .collect();
        ColorImage {
            size: [width, height],
            source_size: Vec2::new(width as f32, height as f32),
            pixels,
        }
    }

    #[test]
    fn small_images_are_one_tile() {
        let level = TiledLevel::lazy(
            "t".to_owned(),
            Arc::new(gradient(TILE_SIDE, TILE_SIDE)),
            TextureOptions::LINEAR,
        );
        assert_eq!((level.columns, level.rows), (1, 1));
    }

    #[test]
    fn large_images_split_with_a_short_last_tile() {
        let level = TiledLevel::lazy(
            "t".to_owned(),
            Arc::new(gradient(TILE_SIDE * 2 + 5, TILE_SIDE + 1)),
            TextureOptions::LINEAR,
        );
        assert_eq!((level.columns, level.rows), (3, 2));
        assert_eq!(level.tiles.len(), 6);
    }

    #[test]
    fn tile_ranges_cover_only_what_is_visible() {
        assert_eq!(tile_range(0.0, 1.0, 5), 0..1);
        assert_eq!(tile_range(2047.0, 2049.0, 5), 0..2);
        assert_eq!(tile_range(4096.0, 6143.0, 5), 2..3);
        assert_eq!(tile_range(-50.0, 100.0, 5), 0..1);
        assert_eq!(tile_range(9000.0, 99999.0, 5), 4..5);
        assert!(tile_range(20000.0, 30000.0, 5).is_empty());
    }

    #[test]
    fn padding_reaches_into_neighbours_but_not_past_the_image() {
        let size = [TILE_SIDE * 2, TILE_SIDE];
        let first = pixel_span(size, 0, 0);
        let padded_first = padded_span(size, &first);
        assert_eq!(padded_first.columns, 0..TILE_SIDE + MARGIN);
        assert_eq!(padded_first.rows, 0..TILE_SIDE);
        let second = pixel_span(size, 0, 1);
        let padded_second = padded_span(size, &second);
        assert_eq!(padded_second.columns, TILE_SIDE - MARGIN..TILE_SIDE * 2);
    }

    #[test]
    fn uv_selects_exactly_the_tiles_own_pixels() {
        let size = [TILE_SIDE * 2, TILE_SIDE];
        let own = pixel_span(size, 0, 1);
        let padded = padded_span(size, &own);
        let uv = own.uv_within(&padded);
        let width = padded.width() as f32;
        assert_eq!(uv.min.x, MARGIN as f32 / width);
        assert_eq!(uv.max.x, 1.0);
        assert_eq!((uv.min.y, uv.max.y), (0.0, 1.0));
    }

    #[test]
    fn crop_copies_the_requested_pixels() {
        let source = gradient(10, 6);
        let span = Span {
            columns: 3..7,
            rows: 2..5,
        };
        let cropped = crop(&source, &span);
        assert_eq!(cropped.size, [4, 3]);
        assert_eq!(cropped.pixels[0], source.pixels[2 * 10 + 3]);
        assert_eq!(cropped.pixels[11], source.pixels[4 * 10 + 6]);
    }

    #[test]
    fn quads_follow_the_orientation() {
        let view = |orientation| View {
            image_rect: Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(40.0, 60.0)),
            points_per_pixel: Vec2::new(2.0, 2.0),
            source_size: [30.0, 20.0],
            orientation,
        };
        let whole = Rect::from_min_size(Pos2::ZERO, Vec2::new(30.0, 20.0));
        let upright = view(Orientation::default()).quad(whole);
        assert_eq!(upright[0], Pos2::new(10.0, 20.0));
        let turned = view(Orientation::default().after(Edit::RotateRight)).quad(whole);
        assert_eq!(turned[0], Pos2::new(10.0 + 40.0, 20.0));
        assert_eq!(turned[1], Pos2::new(10.0 + 40.0, 20.0 + 60.0));
        let mirrored = view(Orientation::default().after(Edit::FlipHorizontal)).quad(whole);
        assert_eq!(mirrored[0], Pos2::new(10.0 + 60.0, 20.0));
    }
}
