use std::sync::Arc;

use eframe::egui::ColorImage;

use crate::app::jobs::{Completed, EditedImage, Outcome, Reporter};
use crate::core::loader::LoadId;
use crate::core::resample::{Filter, resample};

pub struct ResizeRequest {
    pub load: LoadId,
    pub pixels: Arc<ColorImage>,
    pub size: [usize; 2],
    pub filter: Filter,
}

pub fn resize(request: ResizeRequest, reporter: &Reporter) -> Outcome {
    reporter.working("Resizing image...");
    let image = resample(
        bytemuck::cast_slice(&request.pixels.pixels),
        request.pixels.size,
        request.size,
        request.filter,
    )?;
    Ok(Completed::Resized(
        EditedImage {
            load: request.load,
            image,
        },
        request.filter,
    ))
}
