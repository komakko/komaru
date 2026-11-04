use eframe::egui::IconData;
use image::ImageFormat;

const PNG: &[u8] = include_bytes!("../../assets/icon/komaru.png");

pub fn window_icon() -> Option<IconData> {
    let image = image::load_from_memory_with_format(PNG, ImageFormat::Png)
        .ok()?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Some(IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_icon_decodes_to_full_rgba() {
        let icon = window_icon().expect("embedded icon decodes");
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
        assert_eq!((icon.width, icon.height), (256, 256));
    }
}
