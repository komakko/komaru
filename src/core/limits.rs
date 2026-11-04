use std::sync::atomic::{AtomicU64, Ordering};

pub const DEFAULT_ENCODED_BYTES: u64 = 1024 * 1024 * 1024;
pub const DEFAULT_DECODED_BYTES: u64 = 1536 * 1024 * 1024;

static ENCODED_BYTES: AtomicU64 = AtomicU64::new(DEFAULT_ENCODED_BYTES);
static DECODED_BYTES: AtomicU64 = AtomicU64::new(DEFAULT_DECODED_BYTES);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub encoded: u64,
    pub decoded: u64,
}

impl Limits {
    pub const UNLIMITED: Self = Self {
        encoded: u64::MAX,
        decoded: u64::MAX,
    };

    pub fn enforced(self, enforce: bool) -> Self {
        if enforce { self } else { Self::UNLIMITED }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            encoded: DEFAULT_ENCODED_BYTES,
            decoded: DEFAULT_DECODED_BYTES,
        }
    }
}

pub fn apply(limits: Limits) {
    ENCODED_BYTES.store(limits.encoded, Ordering::Release);
    DECODED_BYTES.store(limits.decoded, Ordering::Release);
}

pub fn encoded() -> u64 {
    ENCODED_BYTES.load(Ordering::Acquire)
}

pub fn decoded() -> u64 {
    DECODED_BYTES.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_enforcement_off_lifts_both_limits() {
        let custom = Limits {
            encoded: 10,
            decoded: 20,
        };
        assert_eq!(custom.enforced(true), custom);
        assert_eq!(custom.enforced(false), Limits::UNLIMITED);
    }
}
