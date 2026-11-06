mod action;
mod animation;
mod animation_bar;
mod background;
mod canvas;
mod color_picker;
mod color_tool;
mod commands;
mod copy;
mod crop_draft;
mod crop_job;
mod crop_overlay;
mod drop;
mod file_dialog;
mod flat_button;
mod icons;
mod info_popover;
mod jobs;
mod menu_bar;
mod paste;
mod pin;
mod popover;
mod preferences;
mod prompt;
mod replay_job;
mod resize;
mod resize_job;
mod resize_popover;
mod save;
mod settings;
mod settings_entries;
mod settings_page;
mod settings_store;
mod shortcut_page;
mod shortcut_store;
mod shortcut_text;
mod shortcuts;
mod sidebar;
mod size_warning;
mod status_bar;
mod theme;
mod tiles;
mod toolbar;
mod updates;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;

use eframe::egui::{
    CentralPanel, Color32, ColorImage, Context, Frame, Label, TextureFilter, TextureOptions, Ui,
    Vec2, ViewportCommand, Visuals,
};

use crate::core::config::Config;
use crate::core::crop::PixelRect;
use crate::core::decode::DecodedImage;
use crate::core::edit::{Edit, EditLog};
use crate::core::encode::Format;
use crate::core::info::ImageInfo;
use crate::core::ingest::Origin;
use crate::core::ingest::Payload;
use crate::core::limits;
use crate::core::loader::{
    Activity, Animated, LoadId, LoadResult, Loaded, Loader, LoaderEvent, Mips,
};
use crate::core::recents::{Recent, Recents};
use crate::core::resample::Filter;
use action::{Action, Availability, Scope, Screen};
use animation::Animation;
use background::ViewerBackground;
use canvas::{Canvas, PrimaryDrag};
use color_picker::ColorPicker;
use color_tool::ColorTool;
use copy::{CopyRequest, SourceFile};
use crop_draft::CropDraft;
use crop_job::CropRequest;
use drop::NativeDrop;
use jobs::{Completed, EditedImage, Jobs, Message};
use pin::PinControl;
use preferences::{LossyPolicy, Preferences};
use prompt::Question;
use replay_job::ReplayRequest;
use resize_job::ResizeRequest;
use resize_popover::{Entry, ResizeDraft};
use save::{SaveRequest, suggested_name};
use settings::{Changed, Settings};
use shortcuts::ShortcutBus;
use status_bar::{ImageStatus, Notice};
use tiles::TiledLevel;
use toolbar::ImageTitle;
use updates::Updater;

const RECENTS_SECTION: &str = "recents";

struct ShownImage {
    load: LoadId,
    title: String,
    file_size: Option<u64>,
    lossy_format: Option<Format>,
    info: Arc<ImageInfo>,
    animation: Option<Animation>,
    source: Source,
    pixels: Arc<ColorImage>,
    source_file: Option<SourceFile>,
    levels: Vec<TiledLevel>,
    edits: EditLog,
}

struct Source {
    pixels: Arc<ColorImage>,
    file_size: Option<u64>,
}

enum Discard {
    Clear,
    Quit,
    Load(Box<Loaded>),
}

enum Pending {
    Discard(Discard),
    Job(String, Sender<bool>),
}

pub struct KomaruApp {
    loader: Loader,
    shown: Option<ShownImage>,
    notice: Notice,
    canvas: Canvas,
    pixelated: bool,
    info_open: bool,
    background: ViewerBackground,
    pin: PinControl,
    native_drop: NativeDrop,
    settings: Settings,
    updater: Updater,
    shortcuts: ShortcutBus,
    config: Config,
    jobs: Jobs,
    resize: Option<ResizeDraft>,
    crop: Option<CropDraft>,
    color_tool: Option<ColorTool>,
    prefs: Preferences,
    recents: Recents,
    pending: Option<Pending>,
    settings_unsaved: bool,
    quit_confirmed: bool,
}

impl KomaruApp {
    pub fn new(creation_context: &eframe::CreationContext<'_>, launch_files: Vec<PathBuf>) -> Self {
        let context = creation_context.egui_ctx.clone();
        context.options_mut(|options| options.zoom_with_keyboard = false);
        let (config, config_error) = Config::load();
        let (shortcuts, problems) = shortcut_store::build(config.section(shortcut_store::SECTION));
        let (controls, setting_problem) = settings_store::load(&config);
        let prefs = Preferences::from_controls(&controls);
        limits::apply(prefs.limits);
        theme::apply(&context, prefs.theme);
        let mut recents = Recents::load(config.section(RECENTS_SECTION));
        recents.truncate(if prefs.remember_recent {
            prefs.recent_count
        } else {
            0
        });
        let notice = config_error
            .map(|error| error.to_string())
            .or_else(|| shortcut_store::summary(&problems))
            .or(setting_problem)
            .map_or(Notice::None, Notice::Failed);
        let loader = Loader::spawn(move || context.request_repaint());
        if !launch_files.is_empty() {
            loader
                .requester()
                .ingest(vec![Payload::Files(launch_files)]);
        }
        Self {
            loader,
            shown: None,
            notice,
            canvas: Canvas::default(),
            pixelated: false,
            info_open: false,
            background: ViewerBackground::default(),
            pin: PinControl::detached(),
            native_drop: NativeDrop::default(),
            settings: Settings::new(controls),
            updater: Updater::new(),
            shortcuts,
            config,
            jobs: Jobs::default(),
            resize: None,
            crop: None,
            color_tool: None,
            prefs,
            recents,
            pending: None,
            settings_unsaved: false,
            quit_confirmed: false,
        }
    }

    fn receive_events(&mut self, ctx: &Context) {
        while let Some(event) = self.loader.try_next_event() {
            match event {
                LoaderEvent::Busy(activity) => self.notice = Notice::Busy(activity),
                LoaderEvent::Finished(result) => self.finish_load(result),
                LoaderEvent::Mips(mips) => self.attach_mips(ctx, mips),
                LoaderEvent::Animation(animated) => self.attach_animation(animated),
                LoaderEvent::Idle => self.notice = Notice::None,
            }
        }
    }

    fn attach_mips(&mut self, ctx: &Context, mips: Mips) {
        let Some(shown) = self.shown.as_mut().filter(|shown| shown.load == mips.load) else {
            return;
        };
        let first = shown.levels.len();
        let uploaded = mips.levels.into_iter().enumerate().map(|(offset, image)| {
            TiledLevel::uploaded(
                ctx,
                level_name(&shown.title, first + offset),
                Arc::new(into_color_image(image)),
                TextureOptions::LINEAR,
            )
        });
        shown.levels.extend(uploaded);
    }

    fn attach_animation(&mut self, animated: Animated) {
        if matches!(self.notice, Notice::Working(DECODING_ANIMATION)) {
            self.notice = Notice::None;
        }
        let Some(shown) = self.shown.as_mut().filter(|shown| {
            shown.load.is_same_load(animated.load)
                && shown.animation.is_none()
                && Arc::ptr_eq(&shown.pixels, &shown.source.pixels)
        }) else {
            return;
        };
        let animation = match animated.outcome {
            Ok(animation) if animation.frames.len() > 1 => animation,
            Ok(_) => return,
            Err(error) => {
                self.notice = Notice::Failed(error.to_string());
                return;
            }
        };
        if animation.truncated {
            self.notice = Notice::Done(format!(
                "Animation limited to its first {} frames",
                animation.frames.len()
            ));
        }
        let frames = animation
            .frames
            .into_iter()
            .map(|frame| animation::Frame {
                pixels: Arc::new(into_color_image(frame.image)),
                delay: frame.delay,
            })
            .collect();
        shown.animation = Some(Animation::new(frames, animation.plays));
    }

    fn advance_animation(&mut self, ctx: &Context) {
        let now = Instant::now();
        let Some(animation) = self
            .shown
            .as_mut()
            .and_then(|shown| shown.animation.as_mut())
        else {
            return;
        };
        let was_playing = animation.is_playing();
        let changed = animation.tick(now).is_some();
        let stopped = was_playing && !animation.is_playing();
        if let Some(next_change) = animation.next_change() {
            ctx.request_repaint_after(next_change.saturating_duration_since(now));
        }
        if changed {
            self.show_frame(stopped);
        }
        if stopped {
            self.request_mips();
        }
    }

    fn toggle_playback(&mut self) {
        let Some(animation) = self
            .shown
            .as_mut()
            .and_then(|shown| shown.animation.as_mut())
        else {
            return;
        };
        let restarted = animation.toggle(Instant::now()).is_some();
        let playing = animation.is_playing();
        if restarted {
            self.show_frame(false);
        } else if !playing {
            self.request_mips();
        }
    }

    fn step_frame(&mut self, forward: bool) {
        let stepped = self
            .shown
            .as_mut()
            .and_then(|shown| shown.animation.as_mut())
            .and_then(|animation| animation.step(forward));
        if stepped.is_some() {
            self.show_frame(true);
        }
    }

    fn show_frame(&mut self, with_mips: bool) {
        let Some(shown) = self.shown.as_mut() else {
            return;
        };
        let Some(animation) = &shown.animation else {
            return;
        };
        let pixels = Arc::clone(animation.pixels());
        shown.levels = vec![TiledLevel::lazy(
            level_name(&shown.title, 0),
            Arc::clone(&pixels),
            texture_options(self.pixelated),
        )];
        shown.pixels = pixels;
        shown.load = shown.load.revised();
        if with_mips {
            self.request_mips();
        }
    }

    fn request_mips(&mut self) {
        let Some(shown) = self.shown.as_mut().filter(|shown| shown.levels.len() == 1) else {
            return;
        };
        shown.load = shown.load.revised();
        self.loader.requester().build_mips(
            shown.load,
            shown.pixels.size,
            Arc::new(SharedPixels(Arc::clone(&shown.pixels))),
        );
    }

    fn freeze_animation(&mut self) {
        let Some(shown) = self.shown.as_mut() else {
            return;
        };
        if shown.animation.take().is_some() {
            shown.source = Source {
                pixels: Arc::clone(&shown.pixels),
                file_size: None,
            };
            shown.source_file = None;
            shown.file_size = None;
        }
    }

    fn finish_load(&mut self, result: LoadResult) {
        match result {
            Ok(loaded) => self.offer_load(loaded),
            Err(error) => self.notice = Notice::Failed(error.to_string()),
        }
    }

    fn has_edits_to_lose(&self) -> bool {
        self.prefs.confirm_discard
            && self
                .shown
                .as_ref()
                .is_some_and(|shown| shown.edits.is_modified())
    }

    fn offer_load(&mut self, loaded: Loaded) {
        if !self.has_edits_to_lose() {
            self.show_loaded(loaded);
        } else if matches!(self.pending, Some(Pending::Job(..))) {
            self.notice = Notice::Failed("Answer the open question first".to_owned());
        } else {
            self.pending = Some(Pending::Discard(Discard::Load(Box::new(loaded))));
        }
    }

    fn remember_recent(&mut self, origin: &Origin) {
        if !self.prefs.remember_recent {
            return;
        }
        if let Some(recent) = Recent::from_origin(origin) {
            self.recents.remember(recent, self.prefs.recent_count);
            self.save_recents();
        }
    }

    fn save_recents(&mut self) {
        self.config
            .replace_section(RECENTS_SECTION, self.recents.to_section());
        self.save_config();
    }

    fn save_config(&mut self) {
        if let Err(error) = self.config.save() {
            self.notice = Notice::Failed(error.to_string());
        }
    }

    fn show_loaded(&mut self, loaded: Loaded) {
        {
            {
                self.remember_recent(&loaded.origin);
                self.canvas.reset();
                self.resize = None;
                self.crop = None;
                self.color_tool = None;
                let title = loaded.origin.title();
                let source_file = match (&loaded.origin, loaded.file_size) {
                    (Origin::File(path), Some(size)) => Some(SourceFile {
                        path: path.clone(),
                        size,
                    }),
                    _ => None,
                };
                let image = Arc::new(into_color_image(loaded.base));
                let base = TiledLevel::lazy(
                    level_name(&title, 0),
                    Arc::clone(&image),
                    texture_options(self.pixelated),
                );
                self.loader.requester().build_mips(
                    loaded.id,
                    image.size,
                    Arc::new(SharedPixels(Arc::clone(&image))),
                );
                self.shown = Some(ShownImage {
                    load: loaded.id,
                    title,
                    file_size: loaded.file_size,
                    lossy_format: loaded.lossy_format,
                    info: Arc::new(loaded.info),
                    animation: None,
                    source: Source {
                        pixels: Arc::clone(&image),
                        file_size: loaded.file_size,
                    },
                    pixels: image,
                    source_file,
                    levels: vec![base],
                    edits: EditLog::default(),
                });
                self.notice = Notice::None;
                if let Some(bytes) = loaded.animation_source {
                    self.loader.requester().decode_animation(loaded.id, bytes);
                    self.notice = Notice::Working(DECODING_ANIMATION);
                }
            }
        }
    }

    fn shown_title(&self) -> Option<ImageTitle<'_>> {
        self.shown.as_ref().map(|shown| ImageTitle {
            name: shown.title.as_str(),
            edited: shown.edits.is_modified(),
        })
    }

    fn screen(&self) -> Screen {
        if self.pending.is_some() {
            Screen::Prompt
        } else if self.settings.is_open() {
            Screen::Settings
        } else if self.resize.is_some() {
            Screen::Resize
        } else if self.crop.is_some() {
            Screen::Crop
        } else if self.color_tool.is_some() {
            Screen::ColorPicker
        } else {
            Screen::Viewer
        }
    }

    fn availability(&self) -> Availability {
        Availability {
            image_shown: self.shown.is_some(),
            pin_usable: self.pin.view().is_usable(),
            can_undo: self
                .shown
                .as_ref()
                .is_some_and(|shown| shown.edits.can_undo()),
            can_redo: self
                .shown
                .as_ref()
                .is_some_and(|shown| shown.edits.can_redo()),
            can_play: self
                .shown
                .as_ref()
                .is_some_and(|shown| shown.animation.is_some()),
            can_step: animation_stopped(self.shown.as_ref()),
        }
    }

    fn run_shortcuts(&mut self, ctx: &Context) {
        if self.settings.is_recording() {
            return;
        }
        while let Some(action) = self.shortcuts.take(ctx, self.screen(), self.availability()) {
            self.perform(ctx, action);
        }
    }

    fn save_shortcuts(&mut self) {
        let overrides = shortcut_store::overrides(&self.shortcuts);
        self.config
            .replace_section(shortcut_store::SECTION, overrides);
        self.save_config();
    }

    fn apply_preferences(&mut self, ctx: &Context) {
        self.prefs = Preferences::from_controls(self.settings.controls());
        limits::apply(self.prefs.limits);
        theme::apply(ctx, self.prefs.theme);
        let limit = if self.prefs.remember_recent {
            self.prefs.recent_count
        } else {
            0
        };
        if self.recents.truncate(limit) {
            self.save_recents();
        }
        self.settings_unsaved = true;
    }

    fn save_settings(&mut self) {
        self.settings_unsaved = false;
        if let Err(error) = settings_store::store(&mut self.config, self.settings.controls()) {
            self.notice = Notice::Failed(error.to_string());
        }
    }

    fn question(&self) -> Option<Question> {
        match self.pending.as_ref()? {
            Pending::Discard(discard) => Some(Question {
                text: match discard {
                    Discard::Load(loaded) => {
                        format!("Discard your edits and open {}?", loaded.origin.title())
                    }
                    Discard::Clear | Discard::Quit => "Discard your edits?".to_owned(),
                },
                accept: "Discard",
                decline: "Keep editing",
            }),
            Pending::Job(text, _) => Some(Question {
                text: text.clone(),
                accept: "Continue",
                decline: "Cancel",
            }),
        }
    }

    fn answer(&mut self, ctx: &Context, accepted: bool) {
        match self.pending.take() {
            Some(Pending::Job(_, reply)) => {
                let _ = reply.send(accepted);
            }
            Some(Pending::Discard(discard)) if accepted => match discard {
                Discard::Clear => self.clear(),
                Discard::Quit => {
                    self.quit_confirmed = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
                Discard::Load(loaded) => self.show_loaded(*loaded),
            },
            _ => {}
        }
    }

    fn guard_discard(&mut self, discard: Discard) -> bool {
        if self.pending.is_some() {
            return false;
        }
        if self.has_edits_to_lose() {
            self.pending = Some(Pending::Discard(discard));
            return false;
        }
        true
    }

    fn open_recent(&mut self, index: usize) {
        let payload = match self.recents.get(index) {
            Some(Recent::File(path)) => Payload::Files(vec![path.clone()]),
            Some(Recent::Link(url)) => Payload::Text(url.clone()),
            None => return,
        };
        self.loader.requester().ingest(vec![payload]);
    }

    fn perform(&mut self, ctx: &Context, action: Action) {
        if action.scope() == Scope::Only(Screen::Viewer) {
            self.resize = None;
            self.crop = None;
            self.color_tool = None;
        }
        match action {
            Action::Open => file_dialog::pick_image(self.loader.requester()),
            Action::Paste => {
                self.notice = Notice::Busy(Activity::ReadingClipboard);
                paste::paste_from_clipboard(self.loader.requester());
            }
            Action::CopyImage => self.copy_image(ctx),
            Action::SaveAs => self.save_image(ctx),
            Action::OpenRecent(index) => self.open_recent(index),
            Action::ClearRecents => {
                if self.recents.clear() {
                    self.save_recents();
                }
            }
            Action::Clear => {
                if self.guard_discard(Discard::Clear) {
                    self.clear();
                }
            }
            Action::OpenSettings => self.settings.open(),
            Action::CloseSettings => self.settings.close(),
            Action::AcceptPrompt => self.answer(ctx, true),
            Action::DeclinePrompt => self.answer(ctx, false),
            Action::Quit => ctx.send_viewport_cmd(ViewportCommand::Close),
            Action::FitToWindow => self.canvas.reset(),
            Action::ActualSize => self.canvas.actual_size(ctx.pixels_per_point()),
            Action::TogglePixelated => self.toggle_pixelated(),
            Action::RotateLeft => self.edit(Edit::RotateLeft),
            Action::RotateRight => self.edit(Edit::RotateRight),
            Action::FlipHorizontal => self.edit(Edit::FlipHorizontal),
            Action::FlipVertical => self.edit(Edit::FlipVertical),
            Action::OpenResize => self.open_resize(),
            Action::ApplyResize => self.apply_resize(ctx),
            Action::CancelResize => self.resize = None,
            Action::OpenCrop => self.open_crop(),
            Action::ApplyCrop => self.apply_crop(ctx),
            Action::CancelCrop => self.crop = None,
            Action::OpenColorPicker => self.open_color_picker(),
            Action::CloseColorPicker => self.color_tool = None,
            Action::Undo => self.travel(ctx, |edits| edits.undo_target()),
            Action::Redo => self.travel(ctx, |edits| edits.redo_target()),
            Action::TogglePlayback => self.toggle_playback(),
            Action::PreviousFrame => self.step_frame(false),
            Action::NextFrame => self.step_frame(true),
            Action::TogglePin => {
                if let Some(failure) = self.pin.toggle() {
                    self.notice = Notice::Failed(failure);
                }
            }
        }
        ctx.request_repaint();
    }

    fn report_copy_failure(&mut self) {
        let failure = self
            .color_tool
            .as_mut()
            .and_then(ColorTool::picker_mut)
            .and_then(ColorPicker::take_copy_failure);
        if let Some(failure) = failure {
            self.notice = Notice::Failed(failure.to_string());
        }
    }

    fn clear(&mut self) {
        self.loader.requester().cancel();
        self.shown = None;
        self.notice = Notice::None;
        self.canvas.reset();
    }

    fn copy_image(&mut self, ctx: &Context) {
        let Some(shown) = &self.shown else {
            return;
        };
        if self.jobs.is_running() {
            return;
        }
        let request = shown.copy_request(&self.prefs);
        self.jobs
            .start(ctx, move |reporter| copy::copy(request, reporter));
    }

    fn save_image(&mut self, ctx: &Context) {
        let Some(shown) = &self.shown else {
            return;
        };
        if self.jobs.is_running() {
            return;
        }
        let request = shown.save_request(&self.prefs);
        self.jobs
            .start(ctx, move |reporter| save::save_as(request, reporter));
    }

    fn open_resize(&mut self) {
        let unit = self.prefs.resize_unit;
        self.resize = self
            .shown
            .as_ref()
            .map(|shown| ResizeDraft::new(shown.edits.orientation().size(shown.pixels.size), unit));
    }

    fn apply_resize(&mut self, ctx: &Context) {
        let (Some(shown), Some(draft)) = (&self.shown, &self.resize) else {
            return;
        };
        let Entry::Ready(size) = draft.entry() else {
            return;
        };
        if self.jobs.is_running() {
            return;
        }
        let request = shown.resize_request(size, self.prefs.resize_filter);
        self.resize = None;
        self.freeze_animation();
        self.jobs
            .start(ctx, move |reporter| resize_job::resize(request, reporter));
    }

    fn open_color_picker(&mut self) {
        self.color_tool = Some(ColorTool::default());
    }

    fn open_crop(&mut self) {
        self.crop = self
            .shown
            .as_ref()
            .map(|shown| CropDraft::new(shown.edits.orientation().size(shown.pixels.size)));
    }

    fn apply_crop(&mut self, ctx: &Context) {
        let (Some(shown), Some(draft)) = (&self.shown, &self.crop) else {
            return;
        };
        let Some(rect) = draft.ready() else {
            return;
        };
        if self.jobs.is_running() {
            return;
        }
        let Some(request) = shown.crop_request(rect) else {
            return;
        };
        self.crop = None;
        self.freeze_animation();
        self.jobs
            .start(ctx, move |reporter| crop_job::crop_image(request, reporter));
    }

    fn travel(&mut self, ctx: &Context, target: impl FnOnce(&EditLog) -> Option<usize>) {
        let Some(shown) = &mut self.shown else {
            return;
        };
        let Some(target) = target(&shown.edits) else {
            return;
        };
        if self.jobs.is_running() {
            return;
        }
        if !shown.edits.pixels_change_at(target) {
            shown.edits.seek(target);
            return;
        }
        let steps = shown.edits.steps_before(target);
        if steps.iter().any(|step| step.changes_pixels()) {
            let request = ReplayRequest {
                load: shown.load,
                source: Arc::clone(&shown.source.pixels),
                steps: steps.to_vec(),
                target,
            };
            self.jobs.start(ctx, move |reporter| {
                replay_job::replay_image(request, reporter)
            });
        } else {
            let source = Arc::clone(&shown.source.pixels);
            shown.edits.seek(target);
            self.show_pixels(source);
        }
    }

    fn handle_job_message(&mut self, message: Message) {
        match message {
            Message::Working(text) => self.notice = Notice::Working(text),
            Message::Ask(text, reply) => self.pending = Some(Pending::Job(text, reply)),
            Message::Finished(Err(error)) => self.notice = Notice::Failed(error.to_string()),
            Message::Finished(Ok(completed)) => self.complete(completed),
        }
    }

    fn complete(&mut self, completed: Completed) {
        let notice = match completed {
            Completed::CopiedImage([width, height]) => {
                Notice::Done(format!("Copied {width} x {height} image"))
            }
            Completed::CopiedFile(name) => Notice::Done(format!("Copied {name}")),
            Completed::Saved(name) => Notice::Done(format!("Saved {name}")),
            Completed::Resized(resized, filter) => self.finish_resize(resized, filter),
            Completed::Cropped(cropped, rect) => self.finish_crop(cropped, rect),
            Completed::Replayed(replayed, target) => self.finish_replay(replayed, target),
            Completed::Cancelled => Notice::None,
        };
        if !matches!(self.notice, Notice::Busy(_)) {
            self.notice = notice;
        }
    }

    fn finish_resize(&mut self, resized: EditedImage, filter: Filter) -> Notice {
        let size = [resized.image.width, resized.image.height];
        match self.adopt(resized, |edits| edits.record_resize(size, filter)) {
            Some([width, height]) => Notice::Done(format!("Resized to {width} x {height}")),
            None => Notice::None,
        }
    }

    fn finish_crop(&mut self, cropped: EditedImage, rect: PixelRect) -> Notice {
        match self.adopt(cropped, |edits| edits.record_crop(rect)) {
            Some([width, height]) => Notice::Done(format!("Cropped to {width} x {height}")),
            None => Notice::None,
        }
    }

    fn finish_replay(&mut self, replayed: EditedImage, target: usize) -> Notice {
        match self.adopt(replayed, |edits| edits.seek(target)) {
            Some([width, height]) => Notice::Done(format!("Restored {width} x {height}")),
            None => Notice::None,
        }
    }

    fn adopt(
        &mut self,
        edited: EditedImage,
        record: impl FnOnce(&mut EditLog),
    ) -> Option<[usize; 2]> {
        let shown = self
            .shown
            .as_mut()
            .filter(|shown| shown.load == edited.load)?;
        record(&mut shown.edits);
        shown.load = edited.load;
        self.show_pixels(Arc::new(into_color_image(edited.image)))
    }

    fn show_pixels(&mut self, pixels: Arc<ColorImage>) -> Option<[usize; 2]> {
        let shown = self.shown.as_mut()?;
        let load = shown.load.revised();
        let size = pixels.size;
        shown.levels = vec![TiledLevel::lazy(
            level_name(&shown.title, 0),
            Arc::clone(&pixels),
            texture_options(self.pixelated),
        )];
        shown.file_size = Arc::ptr_eq(&pixels, &shown.source.pixels)
            .then_some(shown.source.file_size)
            .flatten();
        shown.pixels = Arc::clone(&pixels);
        shown.load = load;
        self.loader
            .requester()
            .build_mips(load, size, Arc::new(SharedPixels(pixels)));
        self.canvas.reset();
        self.crop = None;
        Some(shown.edits.orientation().size(size))
    }

    fn edit(&mut self, edit: Edit) {
        if let Some(shown) = &mut self.shown {
            shown.edits.push(edit);
            if self.prefs.refit_after_turn {
                self.canvas.reset();
            }
        }
    }

    fn toggle_pixelated(&mut self) {
        self.pixelated = !self.pixelated;
        if let Some(shown) = &mut self.shown {
            shown.levels[0].set_options(texture_options(self.pixelated));
        }
    }

    fn guard_close_request(&mut self, ctx: &Context) {
        if self.quit_confirmed || !ctx.input(|input| input.viewport().close_requested()) {
            return;
        }
        if !self.has_edits_to_lose() {
            return;
        }
        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        self.guard_discard(Discard::Quit);
    }

    fn show_view(&mut self, ui: &mut Ui) -> (Option<ImageStatus>, Option<Action>) {
        let controls_visible = self.screen() == Screen::Viewer;
        let image_tint = self.background.image_tint();
        let availability = self.availability();
        let Some(shown) = &mut self.shown else {
            centered_message(ui, "Open an image to begin");
            return (None, None);
        };
        let orientation = shown.edits.orientation();
        let primary = if self.color_tool.as_ref().is_some_and(ColorTool::is_armed) {
            PrimaryDrag::Picks
        } else if self.crop.is_some() {
            PrimaryDrag::Reserved
        } else {
            PrimaryDrag::Pans
        };
        let view = self
            .canvas
            .show(ui, &mut shown.levels, orientation, primary, image_tint);
        let action = self
            .crop
            .as_mut()
            .and_then(|draft| crop_overlay::show(ui, &view, draft));
        if let Some(tool) = self.color_tool.as_mut() {
            tool.configure(self.prefs.picker);
            tool.drive(ui, &view, &shown.pixels, orientation);
        }
        let controls = shown
            .animation
            .as_ref()
            .filter(|_| controls_visible)
            .and_then(|animation| {
                animation_bar::show(
                    ui.ctx(),
                    ui.max_rect(),
                    animation,
                    availability,
                    &self.shortcuts,
                )
            });
        let action = action.or(controls);
        let [width, height] = orientation.size(shown.levels[0].size());
        let status = ImageStatus {
            width,
            height,
            file_size: shown.file_size,
            info: Arc::clone(&shown.info),
            readout: view.readout,
        };
        (Some(status), action)
    }
}

impl eframe::App for KomaruApp {
    fn clear_color(&self, _visuals: &Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        theme::apply(&ctx, self.prefs.theme);
        self.receive_events(&ctx);
        self.updater
            .sync(&ctx, self.prefs.check_updates, self.prefs.update_channel);
        self.advance_animation(&ctx);
        self.guard_close_request(&ctx);
        while let Some(message) = self.jobs.poll() {
            self.handle_job_message(message);
        }
        if let Some(payload) = drop::dropped_payload(&ctx) {
            self.loader.requester().ingest(vec![payload]);
        }
        self.pin.attach_once(frame);
        if let Err(error) = self
            .native_drop
            .attach_once(frame, &ctx, self.loader.requester())
        {
            self.notice = Notice::Failed(error.to_string());
        }
        self.pin.poll(&ctx);
        self.run_shortcuts(&ctx);
        let color_tool_was_open = self.color_tool.is_some();

        let toolbar_action = toolbar::show(
            ui,
            self.pin.view(),
            self.shown_title(),
            self.availability(),
            self.pixelated,
            &self.shortcuts,
            &self.recents,
        );
        let below_toolbar = ui.available_rect_before_wrap();
        let status_bar = status_bar::reserve(ui);
        let sidebar_action = sidebar::show(
            ui,
            self.availability(),
            self.resize.as_mut(),
            self.crop.is_some(),
            self.color_tool.as_mut(),
            &mut self.background,
            &self.shortcuts,
        );
        self.report_copy_failure();
        let (viewer_area, (image_status, view_action)) = CentralPanel::default()
            .frame(Frame::NONE.fill(self.background.fill()))
            .show(ui, |ui| (ui.max_rect(), self.show_view(ui)))
            .inner;
        let status_actions = status_bar::show(
            ui,
            status_bar,
            image_status.as_ref(),
            &self.notice,
            self.pixelated,
            self.info_open,
        );
        if let Some(question) = self.question()
            && let Some(accepted) = prompt::show(&ctx, viewer_area, &question)
        {
            self.answer(&ctx, accepted);
        }

        if let Some(action) = toolbar_action.or(sidebar_action).or(view_action) {
            self.perform(&ctx, action);
        }
        if status_actions.pixelation_toggled {
            self.perform(&ctx, Action::TogglePixelated);
        }
        if self.shown.is_none() {
            self.info_open = false;
        }
        if status_actions.info_toggled {
            self.info_open = !self.info_open;
        }
        if status_actions.info_dismissed {
            self.info_open = false;
        }
        if status_actions.cancel_requested {
            self.notice = Notice::None;
            self.loader.requester().cancel();
        }
        if status_actions.actual_size_requested {
            self.perform(&ctx, Action::ActualSize);
        }
        if color_tool_was_open && color_tool::clicked_outside(&ctx, viewer_area) {
            self.perform(&ctx, Action::CloseColorPicker);
        }
        if self.native_drop.is_hovering(&ctx) {
            drop::paint_hover(&ctx, viewer_area);
        }
        let changed: Changed =
            self.settings
                .show(&ctx, below_toolbar, &mut self.shortcuts, &self.updater);
        if changed.install_update {
            self.updater.install(&ctx);
        }
        if changed.shortcuts {
            self.save_shortcuts();
        }
        if changed.controls {
            self.apply_preferences(&ctx);
        }
        if self.settings_unsaved && !ctx.input(|input| input.pointer.any_down()) {
            self.save_settings();
        }
        resize::handle_edge_resize(&ctx);
    }
}

impl ShownImage {
    fn warn_above(&self, policy: LossyPolicy) -> Option<u64> {
        let warns = policy == LossyPolicy::WarnFirst && self.lossy_format.is_some();
        warns.then_some(self.source.file_size).flatten()
    }

    fn copy_request(&self, prefs: &Preferences) -> CopyRequest {
        let unedited_file = if self.edits.is_modified() {
            None
        } else {
            self.source_file.clone()
        };
        CopyRequest {
            file: unedited_file,
            pixels: Arc::clone(&self.pixels),
            orientation: self.edits.orientation(),
            options: prefs.encode,
            warn_above: self.warn_above(prefs.lossy_sources),
        }
    }

    fn resize_request(&self, oriented_size: [usize; 2], filter: Filter) -> ResizeRequest {
        ResizeRequest {
            load: self.load,
            pixels: Arc::clone(&self.pixels),
            size: self.edits.orientation().size(oriented_size),
            filter,
        }
    }

    fn crop_request(&self, oriented: PixelRect) -> Option<CropRequest> {
        let rect = self
            .edits
            .orientation()
            .source_rect(self.pixels.size, oriented)?;
        Some(CropRequest {
            load: self.load,
            pixels: Arc::clone(&self.pixels),
            rect,
        })
    }

    fn forced_format(&self, policy: LossyPolicy) -> Option<Format> {
        let lossy = self.lossy_format?;
        match policy {
            LossyPolicy::KeepSourceFormat => Some(lossy),
            LossyPolicy::ConvertToPng | LossyPolicy::WarnFirst => Some(Format::Png),
        }
    }

    fn save_title(&self) -> String {
        let Some(animation) = &self.animation else {
            return self.title.clone();
        };
        let path = Path::new(&self.title);
        let stem = path.file_stem().unwrap_or(path.as_os_str());
        let extension = path.extension().unwrap_or_default();
        format!(
            "{}-frame-{}.{}",
            stem.to_string_lossy(),
            animation.index() + 1,
            extension.to_string_lossy()
        )
    }

    fn save_request(&self, prefs: &Preferences) -> SaveRequest {
        let directory = self
            .source_file
            .as_ref()
            .and_then(|file| file.path.parent())
            .map(Path::to_owned);
        SaveRequest {
            suggested_name: suggested_name(
                &self.save_title(),
                self.forced_format(prefs.lossy_sources),
            ),
            directory,
            pixels: Arc::clone(&self.pixels),
            orientation: self.edits.orientation(),
            options: prefs.encode,
            warn_above: self.warn_above(prefs.lossy_sources),
        }
    }
}

struct SharedPixels(Arc<ColorImage>);

impl AsRef<[u8]> for SharedPixels {
    fn as_ref(&self) -> &[u8] {
        bytemuck::cast_slice(&self.0.pixels)
    }
}

const DECODING_ANIMATION: &str = "Decoding animation...";

fn animation_stopped(shown: Option<&ShownImage>) -> bool {
    shown
        .and_then(|shown| shown.animation.as_ref())
        .is_some_and(|animation| !animation.is_playing())
}

fn level_name(title: &str, level: usize) -> String {
    format!("{title}#{level}")
}

fn texture_options(pixelated: bool) -> TextureOptions {
    let magnification = if pixelated {
        TextureFilter::Nearest
    } else {
        TextureFilter::Linear
    };
    TextureOptions {
        magnification,
        ..TextureOptions::LINEAR
    }
}

fn into_color_image(image: DecodedImage) -> ColorImage {
    let pixels = match bytemuck::allocation::try_cast_vec::<u8, Color32>(image.premultiplied_rgba) {
        Ok(pixels) => pixels,
        Err((_, bytes)) => bytemuck::cast_slice::<u8, Color32>(&bytes).to_vec(),
    };
    ColorImage {
        size: [image.width, image.height],
        source_size: Vec2::new(image.width as f32, image.height as f32),
        pixels,
    }
}

fn centered_message(ui: &mut Ui, message: &str) {
    ui.centered_and_justified(|ui| {
        ui.add(Label::new(message).selectable(false));
    });
}
