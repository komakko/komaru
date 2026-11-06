use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use rfd::AsyncFileDialog;

use crate::core::ingest::Payload;
use crate::core::loader::LoadRequester;

const IMAGE_EXTENSIONS: [&str; 7] = ["jpg", "jpeg", "png", "webp", "gif", "bmp", "avif"];

static DIALOG_OPEN: AtomicBool = AtomicBool::new(false);

pub fn pick_image(requester: LoadRequester) {
    if DIALOG_OPEN.swap(true, Ordering::AcqRel) {
        return;
    }
    thread::spawn(move || {
        let picked = pollster::block_on(
            AsyncFileDialog::new()
                .add_filter("Images", &IMAGE_EXTENSIONS)
                .pick_file(),
        );
        DIALOG_OPEN.store(false, Ordering::Release);
        if let Some(file) = picked {
            requester.ingest(vec![Payload::Files(vec![file.path().to_owned()])]);
        }
    });
}
