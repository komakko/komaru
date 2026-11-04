use crate::app::jobs::Reporter;
use crate::app::status_bar::format_size;

const GROWTH_FACTOR: u64 = 2;

fn grew_too_much(png_bytes: u64, source_bytes: u64) -> bool {
    png_bytes > source_bytes.saturating_mul(GROWTH_FACTOR)
}

pub fn confirmed(reporter: &Reporter, png_bytes: u64, source_bytes: u64) -> bool {
    !grew_too_much(png_bytes, source_bytes)
        || reporter.confirm(format!(
            "The PNG is {} against {} for the source. Continue?",
            format_size(png_bytes),
            format_size(source_bytes)
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_more_than_double_counts_as_too_much() {
        assert!(!grew_too_much(200, 100));
        assert!(grew_too_much(201, 100));
        assert!(!grew_too_much(10, 100));
    }
}
