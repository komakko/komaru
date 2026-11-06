use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use eframe::egui::Context;

use crate::core::crop::PixelRect;
use crate::core::decode::DecodedImage;
use crate::core::loader::LoadId;
use crate::core::resample::Filter;
use crate::error::JobError;

pub struct EditedImage {
    pub load: LoadId,
    pub image: DecodedImage,
}

pub enum Completed {
    CopiedImage([usize; 2]),
    CopiedFile(String),
    Saved(String),
    SavedFrame(String),
    Resized(EditedImage, Filter),
    Cropped(EditedImage, PixelRect),
    Replayed(EditedImage, usize),
    Cancelled,
}

pub type Outcome = Result<Completed, JobError>;

pub enum Message {
    Working(&'static str),
    Ask(String, Sender<bool>),
    Finished(Outcome),
}

pub struct Reporter {
    sender: Sender<Message>,
    ctx: Context,
}

impl Reporter {
    pub fn confirm(&self, question: String) -> bool {
        let (reply, answer) = mpsc::channel();
        if self.sender.send(Message::Ask(question, reply)).is_err() {
            return false;
        }
        self.ctx.request_repaint();
        answer.recv().unwrap_or(false)
    }

    pub fn working(&self, text: &'static str) {
        let _ = self.sender.send(Message::Working(text));
        self.ctx.request_repaint();
    }
}

#[derive(Default)]
pub struct Jobs {
    pending: Option<Receiver<Message>>,
}

impl Jobs {
    pub fn is_running(&self) -> bool {
        self.pending.is_some()
    }

    pub fn start(
        &mut self,
        ctx: &Context,
        work: impl FnOnce(&Reporter) -> Outcome + Send + 'static,
    ) {
        let (sender, receiver) = mpsc::channel();
        let reporter = Reporter {
            sender,
            ctx: ctx.clone(),
        };
        thread::spawn(move || {
            let outcome = work(&reporter);
            let _ = reporter.sender.send(Message::Finished(outcome));
            reporter.ctx.request_repaint();
        });
        self.pending = Some(receiver);
    }

    pub fn poll(&mut self) -> Option<Message> {
        let message = match self.pending.as_ref()?.try_recv() {
            Ok(message) => message,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Message::Finished(Err(JobError::Stopped)),
        };
        if matches!(message, Message::Finished(_)) {
            self.pending = None;
        }
        Some(message)
    }
}
