use std::thread;

use crate::core::loader::LoadRequester;
use crate::platform::clipboard;

pub fn paste_from_clipboard(requester: LoadRequester) {
    thread::spawn(move || match clipboard::read() {
        Ok(payloads) => requester.ingest(payloads),
        Err(error) => requester.fail(error.into()),
    });
}
