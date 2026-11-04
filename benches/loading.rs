use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use komaru::core::ingest::{Payload, RawPixels};
use komaru::core::loader::{Loaded, Loader, LoaderEvent};

const SIDE: usize = 8192;

struct Timings {
    base: Duration,
    mips: Duration,
}

struct Harness {
    loader: Loader,
    wakes: Receiver<()>,
    source: RawPixels,
}

impl Harness {
    fn new() -> Self {
        let (wake, wakes) = mpsc::channel();
        let loader = Loader::spawn(move || {
            let _ = wake.send(());
        });
        let rgba = (0..SIDE * SIDE * 4)
            .map(|index| (index.wrapping_mul(31) >> 3) as u8)
            .collect();
        Self {
            loader,
            wakes,
            source: RawPixels {
                width: SIDE,
                height: SIDE,
                rgba,
            },
        }
    }

    fn load_once(&self) -> Timings {
        let payload = Payload::Pixels(RawPixels {
            width: self.source.width,
            height: self.source.height,
            rgba: self.source.rgba.clone(),
        });
        let started = Instant::now();
        self.loader.requester().ingest(vec![payload]);
        let loaded = self.wait_for(|event| match event {
            LoaderEvent::Finished(outcome) => Some(outcome.expect("load failed")),
            _ => None,
        });
        let base = started.elapsed();
        self.request_mips(loaded);
        self.wait_for(|event| matches!(event, LoaderEvent::Mips(_)).then_some(()));
        Timings {
            base,
            mips: started.elapsed(),
        }
    }

    fn request_mips(&self, loaded: Loaded) {
        let size = [loaded.base.width, loaded.base.height];
        let pixels = Arc::new(loaded.base.premultiplied_rgba);
        self.loader.requester().build_mips(loaded.id, size, pixels);
    }

    fn wait_for<T>(&self, pick: impl Fn(LoaderEvent) -> Option<T>) -> T {
        loop {
            while let Some(event) = self.loader.try_next_event() {
                if let Some(found) = pick(event) {
                    return found;
                }
            }
            self.wakes.recv().expect("loader stopped");
        }
    }

    fn total(&self, iterations: u64, pick: impl Fn(Timings) -> Duration) -> Duration {
        (0..iterations).map(|_| pick(self.load_once())).sum()
    }
}

fn loading(criterion: &mut Criterion) {
    let harness = Harness::new();
    let mut group = criterion.benchmark_group("load_8192x8192_raw_pixels");
    group.sample_size(10);
    group.bench_function("time_to_base_level", |bencher| {
        bencher.iter_custom(|iterations| harness.total(iterations, |timings| timings.base));
    });
    group.bench_function("time_to_all_levels", |bencher| {
        bencher.iter_custom(|iterations| harness.total(iterations, |timings| timings.mips));
    });
    group.finish();
}

criterion_group!(benches, loading);
criterion_main!(benches);
