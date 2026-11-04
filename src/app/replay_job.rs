use std::sync::Arc;

use eframe::egui::ColorImage;

use crate::app::jobs::{Completed, EditedImage, Outcome, Reporter};
use crate::core::edit::Step;
use crate::core::loader::LoadId;
use crate::core::replay::replay;

pub struct ReplayRequest {
    pub load: LoadId,
    pub source: Arc<ColorImage>,
    pub steps: Vec<Step>,
    pub target: usize,
}

pub fn replay_image(request: ReplayRequest, reporter: &Reporter) -> Outcome {
    reporter.working("Rebuilding image...");
    let image = replay(
        bytemuck::cast_slice(&request.source.pixels),
        request.source.size,
        &request.steps,
    )?;
    Ok(Completed::Replayed(
        EditedImage {
            load: request.load,
            image,
        },
        request.target,
    ))
}
