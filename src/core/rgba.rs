const HUE_SECTORS: f32 = 6.0;
const DEGREES_PER_SECTOR: f32 = 60.0;
pub const FULL_TURN: f32 = 360.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const OPAQUE_BLACK: Self = Self::new(0, 0, 0, 255);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_premultiplied([r, g, b, a]: [u8; 4]) -> Self {
        if a == 0 || a == 255 {
            return Self::new(r, g, b, a);
        }
        let straight = |channel: u8| {
            let scaled = (u32::from(channel) * 255 + u32::from(a) / 2) / u32::from(a);
            scaled.min(255) as u8
        };
        Self::new(straight(r), straight(g), straight(b), a)
    }

    pub fn opaque(self) -> Self {
        Self { a: 255, ..self }
    }

    pub fn parse_hex(text: &str) -> Option<Self> {
        let digits = text.trim().trim_start_matches('#');
        if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        match digits.len() {
            3 => {
                let [r, g, b] = [0, 1, 2].map(|index| nibble(digits, index) * 17);
                Some(Self::new(r, g, b, 255))
            }
            6 => Some(Self::new(
                pair(digits, 0),
                pair(digits, 1),
                pair(digits, 2),
                255,
            )),
            8 => Some(Self::new(
                pair(digits, 0),
                pair(digits, 1),
                pair(digits, 2),
                pair(digits, 3),
            )),
            _ => None,
        }
    }

    pub fn hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsva {
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
    pub alpha: u8,
}

impl Hsva {
    pub fn from_rgba(color: Rgba) -> Self {
        let [r, g, b] = [color.r, color.g, color.b].map(|channel| f32::from(channel) / 255.0);
        let max = r.max(g).max(b);
        let delta = max - r.min(g).min(b);
        let sector = if delta == 0.0 {
            0.0
        } else if max == r {
            ((g - b) / delta).rem_euclid(HUE_SECTORS)
        } else if max == g {
            (b - r) / delta + 2.0
        } else {
            (r - g) / delta + 4.0
        };
        Self {
            hue: sector * DEGREES_PER_SECTOR,
            saturation: if max == 0.0 { 0.0 } else { delta / max },
            value: max,
            alpha: color.a,
        }
    }

    pub fn from_rgba_keeping(color: Rgba, previous: Self) -> Self {
        let derived = Self::from_rgba(color);
        let hue = if derived.saturation == 0.0 || derived.value == 0.0 {
            previous.hue
        } else {
            derived.hue
        };
        let saturation = if derived.value == 0.0 {
            previous.saturation
        } else {
            derived.saturation
        };
        Self {
            hue,
            saturation,
            ..derived
        }
    }

    pub fn rgba(self) -> Rgba {
        let [r, g, b] = rgb_at(self.hue, self.saturation, self.value).map(to_byte);
        Rgba::new(r, g, b, self.alpha)
    }
}

fn rgb_at(hue: f32, saturation: f32, value: f32) -> [f32; 3] {
    let sector = (hue / DEGREES_PER_SECTOR).rem_euclid(HUE_SECTORS);
    let channel = |offset: f32| {
        let position = (offset + sector).rem_euclid(HUE_SECTORS);
        value - value * saturation * position.min(4.0 - position).clamp(0.0, 1.0)
    };
    [channel(5.0), channel(3.0), channel(1.0)]
}

fn to_byte(channel: f32) -> u8 {
    (channel * 255.0).round().clamp(0.0, 255.0) as u8
}

fn nibble(digits: &str, index: usize) -> u8 {
    digits
        .get(index..=index)
        .and_then(|digit| u8::from_str_radix(digit, 16).ok())
        .unwrap_or(0)
}

fn pair(digits: &str, index: usize) -> u8 {
    nibble(digits, index * 2) * 16 + nibble(digits, index * 2 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_accepts_short_long_and_alpha_forms_with_or_without_hash() {
        assert_eq!(Rgba::parse_hex("#2f2e33"), Some(Rgba::new(47, 46, 51, 255)));
        assert_eq!(Rgba::parse_hex("2F2E33"), Some(Rgba::new(47, 46, 51, 255)));
        assert_eq!(Rgba::parse_hex("#fa0"), Some(Rgba::new(255, 170, 0, 255)));
        assert_eq!(
            Rgba::parse_hex("#11223344"),
            Some(Rgba::new(17, 34, 51, 68))
        );
    }

    #[test]
    fn hex_rejects_bad_lengths_and_characters() {
        for bad in [
            "", "#", "#12", "#1234", "#12345", "#1234567", "#gg0000", "+f0000",
        ] {
            assert_eq!(Rgba::parse_hex(bad), None, "{bad}");
        }
    }

    #[test]
    fn hex_output_adds_alpha_only_when_transparent() {
        assert_eq!(Rgba::new(47, 46, 51, 255).hex(), "#2f2e33");
        assert_eq!(Rgba::new(47, 46, 51, 128).hex(), "#2f2e3380");
        assert_eq!(Rgba::new(0, 0, 0, 0).hex(), "#00000000");
    }

    #[test]
    fn hex_round_trips() {
        for color in [Rgba::new(1, 2, 3, 255), Rgba::new(250, 128, 7, 9)] {
            assert_eq!(Rgba::parse_hex(&color.hex()), Some(color));
        }
    }

    #[test]
    fn opaque_and_fully_transparent_pixels_pass_through_exactly() {
        assert_eq!(
            Rgba::from_premultiplied([47, 46, 51, 255]),
            Rgba::new(47, 46, 51, 255)
        );
        assert_eq!(
            Rgba::from_premultiplied([0, 0, 0, 0]),
            Rgba::new(0, 0, 0, 0)
        );
    }

    #[test]
    fn translucent_pixels_are_scaled_back_up_within_rounding() {
        let straight = Rgba::from_premultiplied([100, 50, 0, 128]);
        assert_eq!(straight, Rgba::new(199, 100, 0, 128));
        let clamped = Rgba::from_premultiplied([200, 0, 0, 100]);
        assert_eq!(clamped.r, 255);
    }

    #[test]
    fn primaries_land_on_the_expected_hues() {
        let hue = |r, g, b| Hsva::from_rgba(Rgba::new(r, g, b, 255)).hue;
        assert_eq!(hue(255, 0, 0), 0.0);
        assert_eq!(hue(0, 255, 0), 120.0);
        assert_eq!(hue(0, 0, 255), 240.0);
        assert_eq!(hue(255, 255, 0), 60.0);
    }

    #[test]
    fn every_sampled_colour_survives_the_hsv_round_trip() {
        for r in (0..=255u8).step_by(5) {
            for g in (0..=255u8).step_by(5) {
                for b in (0..=255u8).step_by(5) {
                    let color = Rgba::new(r, g, b, 200);
                    assert_eq!(Hsva::from_rgba(color).rgba(), color);
                }
            }
        }
    }

    #[test]
    fn every_byte_value_survives_for_greys_and_edges() {
        for level in 0..=255u8 {
            for color in [
                Rgba::new(level, level, level, 255),
                Rgba::new(level, 0, 0, 255),
                Rgba::new(0, level, 255, 255),
            ] {
                assert_eq!(Hsva::from_rgba(color).rgba(), color);
            }
        }
    }

    #[test]
    fn hue_is_kept_when_a_colour_loses_its_chroma() {
        let orange = Hsva::from_rgba(Rgba::new(255, 128, 0, 255));
        let grey = Hsva::from_rgba_keeping(Rgba::new(90, 90, 90, 255), orange);
        assert_eq!(grey.hue, orange.hue);
        assert_eq!(grey.saturation, 0.0);
        let black = Hsva::from_rgba_keeping(Rgba::OPAQUE_BLACK, orange);
        assert_eq!(black.hue, orange.hue);
        assert_eq!(black.saturation, orange.saturation);
    }

    #[test]
    fn a_full_turn_of_hue_is_the_same_colour_as_zero() {
        let at = |hue| {
            Hsva {
                hue,
                saturation: 1.0,
                value: 1.0,
                alpha: 255,
            }
            .rgba()
        };
        assert_eq!(at(360.0), at(0.0));
    }
}
