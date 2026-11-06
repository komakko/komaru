use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use eframe::egui::Context;

use crate::core::update::{self, Channel, Release};
use crate::error::UpdateError;
use crate::platform::{elevate, install};

pub enum Status {
    Off,
    Checking,
    Current,
    Available(Release),
    Installing(Release),
    Installed(String),
    Failed(String),
}

enum Outcome {
    Checked(Result<Option<Release>, UpdateError>),
    Installed(Result<String, UpdateError>),
}

pub struct Updater {
    status: Status,
    channel: Option<Channel>,
    generation: u64,
    installed: bool,
    sender: Sender<(u64, Outcome)>,
    receiver: Receiver<(u64, Outcome)>,
}

impl Updater {
    pub fn new() -> Self {
        if let Ok(executable) = std::env::current_exe() {
            update::remove_leftover(&executable);
        }
        let (sender, receiver) = channel();
        Self {
            status: Status::Off,
            channel: None,
            generation: 0,
            installed: install::is_installed(),
            sender,
            receiver,
        }
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn installable(&self) -> bool {
        self.installed
            && matches!(&self.status, Status::Available(release) if release.build.is_some())
    }

    pub fn sync(&mut self, ctx: &Context, enabled: bool, channel: Channel) {
        while let Ok((generation, outcome)) = self.receiver.try_recv() {
            if generation == self.generation {
                self.status = self.settle(outcome);
            }
        }
        if !enabled {
            if self.channel.take().is_some() {
                self.generation += 1;
                self.status = Status::Off;
            }
            return;
        }
        let busy = matches!(self.status, Status::Installing(_) | Status::Installed(_));
        if self.channel != Some(channel) && !busy {
            self.channel = Some(channel);
            self.status = Status::Checking;
            self.spawn(ctx, move || Outcome::Checked(update::check(channel)));
        }
    }

    pub fn install(&mut self, ctx: &Context) {
        let Status::Available(release) = std::mem::replace(&mut self.status, Status::Off) else {
            return;
        };
        if !self.installed {
            self.status = Status::Available(release);
            return;
        }
        self.status = Status::Installing(release.clone());
        self.spawn(ctx, move || Outcome::Installed(apply(&release)));
    }

    fn settle(&self, outcome: Outcome) -> Status {
        match (outcome, &self.status) {
            (Outcome::Installed(Err(UpdateError::Declined)), Status::Installing(release)) => {
                Status::Available(release.clone())
            }
            (Outcome::Checked(Ok(Some(release))), _) => Status::Available(release),
            (Outcome::Checked(Ok(None)), _) => Status::Current,
            (Outcome::Installed(Ok(version)), _) => Status::Installed(version),
            (Outcome::Checked(Err(error)) | Outcome::Installed(Err(error)), _) => {
                Status::Failed(error.to_string())
            }
        }
    }

    fn spawn(&mut self, ctx: &Context, work: impl FnOnce() -> Outcome + Send + 'static) {
        self.generation += 1;
        let generation = self.generation;
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let _ = sender.send((generation, work()));
            ctx.request_repaint();
        });
    }
}

fn apply(release: &Release) -> Result<String, UpdateError> {
    let download = update::download(release)?;
    let executable = update::current_executable()?;
    match update::replace_executable(&executable, download.bytes()) {
        Err(UpdateError::Protected { .. }) => elevate::replace_executable(&executable, &download)?,
        result => result?,
    }
    Ok(release.version.clone())
}
