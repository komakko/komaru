use std::sync::Arc;
use std::thread;

use moxcms::{ColorProfile, DataColorSpace, Layout, Transform8BitExecutor, TransformOptions};

const CHANNELS: usize = 4;
const STRIP_PIXELS: usize = 1 << 16;
const PARALLEL_MIN_BYTES: usize = 4 * 1024 * 1024;
const PROBE_LEVELS: [u8; 5] = [0, 64, 128, 192, 255];
const IDENTITY_TOLERANCE: u8 = 1;

pub fn convert_to_srgb(icc_profile: &[u8], rgba: &mut [u8]) {
    let Some(transform) = srgb_transform(icc_profile) else {
        return;
    };
    if changes_nothing(&*transform) {
        return;
    }
    let workers = if rgba.len() < PARALLEL_MIN_BYTES {
        1
    } else {
        thread::available_parallelism().map_or(1, usize::from)
    };
    let share = (rgba.len() / CHANNELS).div_ceil(workers) * CHANNELS;
    thread::scope(|scope| {
        for part in rgba.chunks_mut(share) {
            let transform = &*transform;
            scope.spawn(move || convert_in_strips(transform, part));
        }
    });
}

fn srgb_transform(icc_profile: &[u8]) -> Option<Arc<Transform8BitExecutor>> {
    let source = ColorProfile::new_from_slice(icc_profile).ok()?;
    if source.color_space != DataColorSpace::Rgb {
        return None;
    }
    source
        .create_transform_8bit(
            Layout::Rgba,
            &ColorProfile::new_srgb(),
            Layout::Rgba,
            TransformOptions::default(),
        )
        .ok()
}

fn changes_nothing(transform: &Transform8BitExecutor) -> bool {
    let probe: Vec<u8> = PROBE_LEVELS
        .iter()
        .flat_map(|&red| {
            PROBE_LEVELS.iter().flat_map(move |&green| {
                PROBE_LEVELS
                    .iter()
                    .flat_map(move |&blue| [red, green, blue, u8::MAX])
            })
        })
        .collect();
    let mut converted = vec![0; probe.len()];
    transform.transform(&probe, &mut converted).is_ok()
        && probe
            .iter()
            .zip(&converted)
            .all(|(before, after)| before.abs_diff(*after) <= IDENTITY_TOLERANCE)
}

fn convert_in_strips(transform: &Transform8BitExecutor, pixels: &mut [u8]) {
    let mut converted = vec![0; STRIP_PIXELS * CHANNELS];
    for strip in pixels.chunks_mut(STRIP_PIXELS * CHANNELS) {
        let output = &mut converted[..strip.len()];
        if transform.transform(strip, output).is_err() {
            return;
        }
        strip.copy_from_slice(output);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_bytes(profile: &ColorProfile) -> Vec<u8> {
        profile.encode().unwrap()
    }

    fn pixels() -> Vec<u8> {
        vec![200, 100, 50, 255, 10, 240, 128, 77]
    }

    #[test]
    fn srgb_profiles_change_nothing() {
        let mut converted = pixels();
        convert_to_srgb(&profile_bytes(&ColorProfile::new_srgb()), &mut converted);
        assert_eq!(converted, pixels());
    }

    #[test]
    fn wide_gamut_profiles_are_converted_and_keep_alpha() {
        let mut converted = pixels();
        convert_to_srgb(
            &profile_bytes(&ColorProfile::new_display_p3()),
            &mut converted,
        );
        assert_ne!(converted[..3], pixels()[..3]);
        assert_eq!(converted[3], 255);
        assert_eq!(converted[7], 77);
    }

    #[test]
    fn unreadable_or_non_rgb_profiles_leave_pixels_alone() {
        let mut converted = pixels();
        convert_to_srgb(b"garbage", &mut converted);
        assert_eq!(converted, pixels());
        convert_to_srgb(
            &profile_bytes(&ColorProfile::new_gray_with_gamma(2.2)),
            &mut converted,
        );
        assert_eq!(converted, pixels());
    }

    #[test]
    fn large_images_convert_in_parallel_like_small_ones() {
        let profile = profile_bytes(&ColorProfile::new_display_p3());
        let pattern = pixels();
        let mut small = pattern.clone();
        convert_to_srgb(&profile, &mut small);
        let mut large: Vec<u8> = pattern
            .iter()
            .copied()
            .cycle()
            .take(PARALLEL_MIN_BYTES + 8)
            .collect();
        convert_to_srgb(&profile, &mut large);
        assert_eq!(large[..8], small[..]);
        assert_eq!(large[large.len() - 8..], small[..]);
    }
}
