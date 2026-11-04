use exif::{In, Reader, Value};
use image::metadata::Orientation;
use image::{ExtendedColorType, ImageFormat};
use moxcms::{ColorProfile, ProfileText};

const EXIF_HEADER: &[u8] = b"Exif\0\0";
const MAX_FIELDS: usize = 96;
const MAX_VALUE_CHARS: usize = 160;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageInfo {
    pub format: Option<&'static str>,
    pub color: Option<String>,
    pub profile: Option<String>,
    pub orientation: Option<&'static str>,
    pub metadata: Vec<(String, String)>,
}

pub struct Probe<'a> {
    pub format: ImageFormat,
    pub color: ExtendedColorType,
    pub orientation: Orientation,
    pub icc_profile: Option<&'a [u8]>,
    pub exif: Option<&'a [u8]>,
}

impl ImageInfo {
    pub fn from_probe(probe: &Probe) -> Self {
        Self {
            format: format_name(probe.format),
            color: Some(color_text(probe.color)),
            profile: probe.icc_profile.map(profile_name),
            orientation: orientation_text(probe.orientation),
            metadata: probe.exif.map(exif_fields).unwrap_or_default(),
        }
    }
}

fn format_name(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Jpeg => Some("JPEG"),
        ImageFormat::Png => Some("PNG"),
        ImageFormat::WebP => Some("WebP"),
        ImageFormat::Gif => Some("GIF"),
        ImageFormat::Bmp => Some("BMP"),
        ImageFormat::Avif => Some("AVIF"),
        _ => None,
    }
}

fn color_text(color: ExtendedColorType) -> String {
    let channels = color.channel_count();
    let bits = color.bits_per_pixel() / u16::from(channels.max(1));
    let model = match color {
        ExtendedColorType::Cmyk8 | ExtendedColorType::Cmyk16 => "CMYK",
        _ => match channels {
            1 => "Grayscale",
            2 => "Grayscale + alpha",
            3 => "RGB",
            _ => "RGB + alpha",
        },
    };
    format!("{model}, {bits} bit")
}

fn profile_name(icc_profile: &[u8]) -> String {
    ColorProfile::new_from_slice(icc_profile)
        .ok()
        .and_then(|profile| profile.description)
        .and_then(|text| text_of(&text))
        .unwrap_or_else(|| "Embedded profile".to_owned())
}

fn text_of(text: &ProfileText) -> Option<String> {
    let value = match text {
        ProfileText::PlainString(value) => value.clone(),
        ProfileText::Localizable(entries) => entries.first()?.value.clone(),
        ProfileText::Description(description) => description.ascii_string.clone(),
    };
    let trimmed = value.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn orientation_text(orientation: Orientation) -> Option<&'static str> {
    match orientation {
        Orientation::NoTransforms => None,
        Orientation::Rotate90 => Some("Rotated 90° clockwise"),
        Orientation::Rotate180 => Some("Rotated 180°"),
        Orientation::Rotate270 => Some("Rotated 90° counterclockwise"),
        Orientation::FlipHorizontal => Some("Mirrored horizontally"),
        Orientation::FlipVertical => Some("Mirrored vertically"),
        Orientation::Rotate90FlipH => Some("Rotated 90° clockwise, mirrored"),
        Orientation::Rotate270FlipH => Some("Rotated 90° counterclockwise, mirrored"),
    }
}

fn exif_fields(raw: &[u8]) -> Vec<(String, String)> {
    let tiff = raw.strip_prefix(EXIF_HEADER).unwrap_or(raw);
    let Ok(exif) = Reader::new().read_raw(tiff.to_vec()) else {
        return Vec::new();
    };
    exif.fields()
        .filter(|field| field.ifd_num == In::PRIMARY)
        .filter(|field| !matches!(field.value, Value::Undefined(..) | Value::Unknown(..)))
        .filter_map(|field| {
            let name = field.tag.description()?;
            let value = field.display_value().with_unit(&exif).to_string();
            let value = value.trim_matches('"').trim().to_owned();
            (!value.is_empty() && value.chars().count() <= MAX_VALUE_CHARS)
                .then(|| (name.to_owned(), value))
        })
        .take(MAX_FIELDS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(format: ImageFormat) -> Probe<'static> {
        Probe {
            format,
            color: ExtendedColorType::Rgba8,
            orientation: Orientation::NoTransforms,
            icc_profile: None,
            exif: None,
        }
    }

    #[test]
    fn describes_color_type_and_format() {
        let info = ImageInfo::from_probe(&probe(ImageFormat::Png));
        assert_eq!(info.format, Some("PNG"));
        assert_eq!(info.color.as_deref(), Some("RGB + alpha, 8 bit"));
        assert_eq!(info.orientation, None);
    }

    #[test]
    fn names_an_embedded_profile() {
        let encoded = ColorProfile::new_display_p3().encode().unwrap();
        let info = ImageInfo::from_probe(&Probe {
            icc_profile: Some(&encoded),
            ..probe(ImageFormat::Png)
        });
        assert_ne!(info.profile.as_deref(), None);
        let broken = ImageInfo::from_probe(&Probe {
            icc_profile: Some(b"junk"),
            ..probe(ImageFormat::Png)
        });
        assert_eq!(broken.profile.as_deref(), Some("Embedded profile"));
    }

    #[test]
    fn reads_exif_with_or_without_the_header() {
        let tiff = [
            b'M', b'M', 0, 42, 0, 0, 0, 8, 0, 1, 0x01, 0x0F, 0, 2, 0, 0, 0, 2, b'A', 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let with_header = [EXIF_HEADER, &tiff].concat();
        for raw in [tiff.to_vec(), with_header] {
            let fields = exif_fields(&raw);
            assert_eq!(
                fields,
                vec![(
                    "Manufacturer of image input equipment".to_owned(),
                    "A".to_owned()
                )]
            );
        }
        assert!(exif_fields(b"junk").is_empty());
    }
}
