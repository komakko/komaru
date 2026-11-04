use std::sync::Arc;

use eframe::egui::ColorImage;

use crate::app::jobs::{Completed, EditedImage, Outcome, Reporter};
use crate::core::crop::{PixelRect, crop};
use crate::core::loader::LoadId;

pub struct CropRequest {
    pub load: LoadId,
    pub pixels: Arc<ColorImage>,
    pub rect: PixelRect,
}

pub fn crop_image(request: CropRequest, reporter: &Reporter) -> Outcome {
    reporter.working("Cropping image...");
    let image = crop(
        bytemuck::cast_slice(&request.pixels.pixels),
        request.pixels.size,
        request.rect,
    )?;
    Ok(Completed::Cropped(
        EditedImage {
            load: request.load,
            image,
        },
        request.rect,
    ))
}
