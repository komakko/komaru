use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_2, PI};

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

const HALF_DISC_STEPS: usize = 10;
const HALF_DISC_RADIUS: f32 = 0.7;
const STROKE_WIDTH: f32 = 1.6;
const HEAVY_STROKE_WIDTH: f32 = 2.2;
const ARROW_LENGTH: f32 = 0.55;
const ARROW_SPREAD: f32 = 0.6;
const ARC_RADIUS: f32 = 0.8;
const ROTATE_ARC_START: f32 = -0.87;
const RESET_ARC_START: f32 = 2.35;
const ARC_SWEEP: f32 = -4.71;
const ARC_STEPS: usize = 16;
const LINK_SLIDE: f32 = 0.4;
const LINK_HALF_LENGTH: f32 = 0.3;
const LINK_RADIUS: f32 = 0.34;
const LINK_CAP_STEPS: usize = 8;
const DROPPER_BULB: Unit = (0.6, -0.6);
const DROPPER_BULB_RADIUS: f32 = 0.4;
const PIN_HEAD: Unit = (0.55, -0.55);
const PIN_HEAD_RADIUS: f32 = 0.89;

type Unit = (f32, f32);

#[derive(Clone, Copy)]
pub enum Icon {
    Close,
    Minimize,
    Maximize,
    Restore,
    Pin { pinned: bool },
    Resize,
    Crop,
    Rotate,
    Flip,
    ColorPicker,
    Pixels,
    Link,
    Reset,
    Copy,
    Info,
    Play,
    Pause,
    StepBack,
    StepForward,
    Opacity,
}

pub fn paint(painter: &Painter, center: Pos2, reach: f32, icon: Icon, color: Color32) {
    let sketch = Sketch {
        painter,
        center,
        reach,
        color,
        stroke: Stroke::new(STROKE_WIDTH, color),
    };
    match icon {
        Icon::Close => {
            sketch.line(&[(-1.0, -1.0), (1.0, 1.0)]);
            sketch.line(&[(-1.0, 1.0), (1.0, -1.0)]);
        }
        Icon::Minimize => Sketch {
            stroke: Stroke::new(HEAVY_STROKE_WIDTH, color),
            ..sketch
        }
        .line(&[(-1.0, 0.78), (1.0, 0.78)]),
        Icon::Maximize => sketch.rect((-1.0, -1.0), (1.0, 1.0)),
        Icon::Restore => {
            sketch.rect((-1.0, -0.45), (0.45, 1.0));
            sketch.line(&[
                (-0.45, -0.45),
                (-0.45, -1.0),
                (1.0, -1.0),
                (1.0, 0.45),
                (0.45, 0.45),
            ]);
        }
        Icon::Pin { pinned } => {
            sketch.line(&[(-1.0, 1.0), PIN_HEAD]);
            sketch.circle(PIN_HEAD, PIN_HEAD_RADIUS, pinned);
        }
        Icon::Resize => {
            sketch.rect((-1.0, -0.1), (0.1, 1.0));
            sketch.arrow((-0.1, 0.1), (1.0, -1.0));
        }
        Icon::Crop => {
            sketch.line(&[(-0.45, -1.0), (-0.45, 0.45), (1.0, 0.45)]);
            sketch.line(&[(-1.0, -0.45), (0.45, -0.45), (0.45, 1.0)]);
        }
        Icon::Rotate => sketch.rotation_arrow(ROTATE_ARC_START),
        Icon::Reset => sketch.rotation_arrow(RESET_ARC_START),
        Icon::Link => sketch.chain_links(),
        Icon::Copy => {
            sketch.rect((-1.0, -0.35), (0.35, 1.0));
            sketch.line(&[
                (-0.35, -0.35),
                (-0.35, -1.0),
                (1.0, -1.0),
                (1.0, 0.35),
                (0.35, 0.35),
            ]);
        }
        Icon::Flip => {
            sketch.line(&[(0.0, -1.0), (0.0, 1.0)]);
            sketch.arrow((-0.2, 0.0), (-1.0, 0.0));
            sketch.arrow((0.2, 0.0), (1.0, 0.0));
        }
        Icon::ColorPicker => {
            sketch.line(&[(-1.0, 1.0), (0.1, -0.1)]);
            sketch.line(&[(-0.15, -0.5), (0.5, 0.15)]);
            sketch.circle(DROPPER_BULB, DROPPER_BULB_RADIUS, true);
        }
        Icon::Info => {
            sketch.circle((0.0, 0.0), 1.0, false);
            sketch.circle((0.0, -0.5), 0.11, true);
            sketch.line(&[(0.0, -0.1), (0.0, 0.55)]);
        }
        Icon::Opacity => {
            sketch.circle((0.0, 0.0), 1.0, false);
            let half: Vec<Unit> = (0..=HALF_DISC_STEPS)
                .map(|step| {
                    let angle = FRAC_PI_2 + PI * step as f32 / HALF_DISC_STEPS as f32;
                    (
                        HALF_DISC_RADIUS * angle.cos(),
                        HALF_DISC_RADIUS * angle.sin(),
                    )
                })
                .collect();
            sketch.polygon(&half);
        }
        Icon::Play => sketch.polygon(&[(-0.6, -1.0), (-0.6, 1.0), (0.95, 0.0)]),
        Icon::Pause => {
            sketch.block((-0.75, -1.0), (-0.2, 1.0));
            sketch.block((0.2, -1.0), (0.75, 1.0));
        }
        Icon::StepBack => {
            sketch.block((-1.0, -0.9), (-0.7, 0.9));
            sketch.polygon(&[(0.95, -0.9), (0.95, 0.9), (-0.5, 0.0)]);
        }
        Icon::StepForward => {
            sketch.block((0.7, -0.9), (1.0, 0.9));
            sketch.polygon(&[(-0.95, -0.9), (-0.95, 0.9), (0.5, 0.0)]);
        }
        Icon::Pixels => {
            sketch.rect((-1.0, -1.0), (1.0, 1.0));
            sketch.block((-1.0, -1.0), (0.0, 0.0));
            sketch.block((0.0, 0.0), (1.0, 1.0));
        }
    }
}

#[derive(Clone, Copy)]
struct Sketch<'a> {
    painter: &'a Painter,
    center: Pos2,
    reach: f32,
    color: Color32,
    stroke: Stroke,
}

impl Sketch<'_> {
    fn point(&self, (x, y): Unit) -> Pos2 {
        self.center + Vec2::new(x, y) * self.reach
    }

    fn span(&self, min: Unit, max: Unit) -> Rect {
        Rect::from_min_max(self.point(min), self.point(max))
    }

    fn line(&self, points: &[Unit]) {
        let points = points.iter().map(|&unit| self.point(unit)).collect();
        self.painter.add(Shape::line(points, self.stroke));
    }

    fn rect(&self, min: Unit, max: Unit) {
        self.painter
            .rect_stroke(self.span(min, max), 0.0, self.stroke, StrokeKind::Middle);
    }

    fn block(&self, min: Unit, max: Unit) {
        self.painter
            .rect_filled(self.span(min, max), 0.0, self.color);
    }

    fn circle(&self, center: Unit, radius: f32, filled: bool) {
        let center = self.point(center);
        let radius = radius * self.reach;
        if filled {
            self.painter.circle_filled(center, radius, self.color);
        } else {
            self.painter.circle_stroke(center, radius, self.stroke);
        }
    }

    fn arrow(&self, from: Unit, to: Unit) {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let length = dx.hypot(dy);
        self.line(&[from, to]);
        self.arrow_head(to, (dx / length, dy / length));
    }

    fn arrow_head(&self, tip: Unit, heading: Unit) {
        let barb = |angle: f32| {
            let (sin, cos) = angle.sin_cos();
            (
                tip.0 + ARROW_LENGTH * (heading.1 * sin - heading.0 * cos),
                tip.1 - ARROW_LENGTH * (heading.0 * sin + heading.1 * cos),
            )
        };
        self.line(&[barb(ARROW_SPREAD), tip, barb(-ARROW_SPREAD)]);
    }

    fn polygon(&self, points: &[Unit]) {
        let points = points.iter().map(|&unit| self.point(unit)).collect();
        self.painter
            .add(Shape::convex_polygon(points, self.color, Stroke::NONE));
    }

    fn closed_line(&self, points: &[Unit]) {
        let points = points.iter().map(|&unit| self.point(unit)).collect();
        self.painter.add(Shape::closed_line(points, self.stroke));
    }

    fn chain_links(&self) {
        for side in [-1.0, 1.0] {
            self.capsule(side * LINK_SLIDE);
        }
    }

    fn capsule(&self, slide: f32) {
        let outline: Vec<Unit> = [1.0, -1.0]
            .into_iter()
            .flat_map(|sign| {
                (0..=LINK_CAP_STEPS).map(move |step| {
                    let angle = -FRAC_PI_2 + PI * step as f32 / LINK_CAP_STEPS as f32;
                    let (sin, cos) = angle.sin_cos();
                    (
                        sign * (LINK_HALF_LENGTH + LINK_RADIUS * cos),
                        sign * LINK_RADIUS * sin,
                    )
                })
            })
            .map(|(along, across)| {
                (
                    (slide + along + across) * FRAC_1_SQRT_2,
                    (across - slide - along) * FRAC_1_SQRT_2,
                )
            })
            .collect();
        self.closed_line(&outline);
    }

    fn rotation_arrow(&self, start: f32) {
        let angle_at = |step: usize| start + ARC_SWEEP * step as f32 / ARC_STEPS as f32;
        let on_arc = |angle: f32| {
            let (sin, cos) = angle.sin_cos();
            (cos * ARC_RADIUS, sin * ARC_RADIUS)
        };
        let arc: Vec<Unit> = (0..=ARC_STEPS).map(|step| on_arc(angle_at(step))).collect();
        self.line(&arc);
        let (sin, cos) = angle_at(ARC_STEPS).sin_cos();
        self.arrow_head(on_arc(angle_at(ARC_STEPS)), (sin, -cos));
    }
}
