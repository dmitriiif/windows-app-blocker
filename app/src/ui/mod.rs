//! The control panel and setup wizard.

mod control;
mod settings;
mod setup;
pub mod theme;
pub mod timeline;

use crate::config::{self, Config};
use crate::install;
use crate::win;
use crate::paths::{self, APP_NAME};
use crate::task::{self, TaskInfo};
use egui::{vec2, Align2, Color32, Frame, Id, LayerId, Margin, Order, RichText, Rounding, Sense, Stroke, ViewportCommand};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use timeline::TimelineState;

pub fn run() -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size([1000.0, 720.0])
            .with_min_inner_size([760.0, 560.0])
            .with_decorations(false)
            .with_icon(theme::window_icon()),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(APP_NAME, options, Box::new(|cc| Ok(Box::new(App::new(&cc.egui_ctx)))))
        .map_err(|e| e.to_string())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Setup,
    Control,
}

pub(crate) struct SetupState {
    step: usize,
    /// Lock choices kept from a previous installation; they cannot be changed.
    locked_policies: bool,
    /// The previous installation's settings, used to decide what is allowed right now.
    previous: Option<Config>,
    notice: Option<String>,
}

enum ConfirmAction {
    Install,
    Uninstall,
    DiscardAndClose,
}

enum Modal {
    Message { title: String, message: String, error: bool, close_app: bool },
    Confirm { title: String, message: String, confirm: String, danger: bool, action: ConfirmAction },
}

enum JobDone {
    Installed,
    Protection(Option<TaskInfo>),
    Updated,
    Uninstalled,
    TestNotification,
}

struct Job {
    label: String,
    receiver: Receiver<Result<JobDone, String>>,
}

/// Polls the scheduled task in the background so the window never stalls on `schtasks.exe`.
struct TaskWatcher {
    state: Arc<Mutex<Option<Option<TaskInfo>>>>,
    wake: Sender<()>,
}

impl TaskWatcher {
    fn new(ctx: egui::Context) -> TaskWatcher {
        let state = Arc::new(Mutex::new(None));
        let (wake, rx) = mpsc::channel::<()>();
        let shared = Arc::clone(&state);
        std::thread::spawn(move || loop {
            let info = task::query();
            *shared.lock().expect("task state") = Some(info);
            ctx.request_repaint();
            if let Err(RecvTimeoutError::Disconnected) = rx.recv_timeout(Duration::from_secs(3)) {
                break;
            }
        });
        TaskWatcher { state, wake }
    }

    /// `None` until the first query finishes; `Some(None)` when the task does not exist.
    fn get(&self) -> Option<Option<TaskInfo>> {
        self.state.lock().expect("task state").clone()
    }

    fn set(&self, info: Option<TaskInfo>) {
        *self.state.lock().expect("task state") = Some(info);
        let _ = self.wake.send(());
    }
}

pub struct App {
    screen: Screen,
    setup: SetupState,
    /// The configuration on disk (control panel only).
    saved: Option<Config>,
    /// The configuration being edited.
    draft: Config,
    load_error: Option<String>,
    timeline: TimelineState,
    selected_apps: Vec<String>,
    task: TaskWatcher,
    modal: Option<Modal>,
    toast: Option<(String, Instant)>,
    job: Option<Job>,
    last_reload: Instant,
    allow_close: bool,
    settings_open: bool,
    /// This copy of the program differs from the installed one.
    update_available: bool,
    /// Setup choices last written to disk, and when the draft last changed since then.
    setup_persisted: Option<Config>,
    setup_changed_at: Option<Instant>,
}

impl App {
    fn new(ctx: &egui::Context) -> App {
        theme::install_fonts(ctx);
        let path = paths::config_path();
        let (existing, read_error) = if path.exists() {
            match config::load(&path) {
                Ok(config) => (Some(config), None),
                Err(e) => (None, Some(e)),
            }
        } else {
            (None, None)
        };
        theme::apply(ctx, existing.as_ref().map(|c| c.preferences.theme).unwrap_or_default());
        let legacy_task = task::query().is_some_and(|t| t.is_legacy());

        let mut app = App {
            screen: Screen::Setup,
            setup: SetupState { step: 0, locked_policies: false, previous: None, notice: None },
            saved: None,
            draft: Config::default(),
            load_error: None,
            timeline: TimelineState::default(),
            selected_apps: Vec::new(),
            task: TaskWatcher::new(ctx.clone()),
            modal: None,
            toast: None,
            job: None,
            last_reload: Instant::now(),
            allow_close: false,
            settings_open: false,
            update_available: false,
            setup_persisted: None,
            setup_changed_at: None,
        };

        match existing {
            Some(config) if config.setup_completed && install::is_installed() => app.open_control(config),
            Some(config) => {
                let completed = config.setup_completed;
                app.setup.locked_policies = completed;
                app.setup.previous = completed.then(|| config.clone());
                app.setup.notice = Some(if legacy_task {
                    "Upgrading from the previous version. Your apps, schedule and lock choices have been carried over.".into()
                } else if completed {
                    "Welcome back. Your apps, schedule and lock choices were kept from your previous installation.".into()
                } else {
                    "Your earlier setup was not finished. Pick up where you left off.".into()
                });
                app.draft = config;
            }
            None => {
                if let Some(error) = read_error {
                    app.setup.notice = Some(format!("Your previous settings could not be read, so setup starts fresh. ({error})"));
                }
            }
        }
        if app.screen == Screen::Setup {
            if !config::is_valid_sid(&app.draft.target_user_sid) {
                app.draft.target_user_sid = win::current_user_sid().unwrap_or_default();
            }
            app.setup_persisted = Some(app.draft.clone());
        }
        app
    }

    /// Writes the setup choices to disk about a second after the last change (or right away when
    /// `now` is set), so closing the window before installing does not lose them.
    fn persist_setup_progress(&mut self, now: bool) {
        if self.screen != Screen::Setup || self.job.is_some() || self.setup_persisted.as_ref() == Some(&self.draft) {
            self.setup_changed_at = None;
            return;
        }
        let changed_at = *self.setup_changed_at.get_or_insert_with(Instant::now);
        if !now && changed_at.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.setup_changed_at = None;
        let mut progress = self.draft.clone();
        if let (Some(previous), true) = (&self.setup.previous, self.setup.locked_policies) {
            progress.policies = previous.policies;
        }
        if progress.validate_schedule().is_err() || !config::is_valid_sid(&progress.target_user_sid) {
            return;
        }
        if let Err(message) = install::save_setup_progress(&progress) {
            self.toast(format!("Could not save your setup progress: {message}"));
        }
        // Recorded even on failure, so a broken disk does not cause a retry every frame.
        self.setup_persisted = Some(self.draft.clone());
    }

    fn open_control(&mut self, config: Config) {
        self.draft = config.clone();
        self.saved = Some(config);
        self.screen = Screen::Control;
        self.selected_apps.clear();
        self.update_available = copy_differs_from_installed();
    }

    fn is_dirty(&self) -> bool {
        self.saved.as_ref().is_some_and(|s| {
            s.executables != self.draft.executables || s.days != self.draft.days || s.schedule_mode != self.draft.schedule_mode
        })
    }

    /// Re-reads the configuration from disk, keeping unsaved edits.
    fn reload_saved(&mut self) {
        self.last_reload = Instant::now();
        match config::load(&paths::config_path()) {
            Ok(config) => {
                if !self.is_dirty() {
                    self.draft = config.clone();
                }
                self.saved = Some(config);
                self.load_error = None;
            }
            Err(e) => self.load_error = Some(e),
        }
    }

    fn error(&mut self, title: &str, message: impl Into<String>) {
        self.modal = Some(Modal::Message { title: title.into(), message: message.into(), error: true, close_app: false });
    }

    fn toast(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now()));
    }

    fn start_job(&mut self, label: &str, work: impl FnOnce() -> Result<JobDone, String> + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(work());
        });
        self.job = Some(Job { label: label.into(), receiver });
    }

    fn poll_job(&mut self) {
        let Some(job) = &self.job else { return };
        let result = match job.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err("The operation stopped unexpectedly.".into()),
        };
        self.job = None;
        match result {
            Ok(JobDone::Installed) => {
                self.task.set(task::query());
                match config::load(&paths::config_path()) {
                    Ok(config) => {
                        self.open_control(config);
                        self.toast("Setup finished. Turn protection on when you are ready.");
                    }
                    Err(e) => self.error("Setup finished with a problem", e),
                }
            }
            Ok(JobDone::Protection(info)) => self.task.set(info),
            Ok(JobDone::Updated) => {
                self.task.set(task::query());
                self.update_available = false;
                self.toast("The installed app has been updated.");
            }
            Ok(JobDone::Uninstalled) => {
                self.modal = Some(Modal::Message {
                    title: "Uninstall started".into(),
                    message: format!(
                        "{APP_NAME} has been turned off and will finish removing itself when this window closes.\n\nYour settings stay in {} for next time.",
                        paths::data_dir().display()
                    ),
                    error: false,
                    close_app: true,
                });
            }
            Ok(JobDone::TestNotification) => self.toast("Test notification sent. It should appear in the corner of your screen."),
            Err(message) => self.error("Something went wrong", message),
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.allow_close {
            return;
        }
        if self.job.is_some() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.toast("Please wait for the current operation to finish.");
        } else if self.screen == Screen::Setup {
            self.persist_setup_progress(true);
        } else if self.screen == Screen::Control && self.is_dirty() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.modal = Some(Modal::Confirm {
                title: "Discard unsaved changes?".into(),
                message: "You have changed your apps or schedule without saving. Close anyway and lose those changes?".into(),
                confirm: "Discard and close".into(),
                danger: true,
                action: ConfirmAction::DiscardAndClose,
            });
        }
    }

    fn show_modal(&mut self, ctx: &egui::Context) {
        let Some(modal) = &self.modal else { return };
        dim_background(ctx, "modal-dim");
        let mut dismissed = false;
        let mut confirmed = None;
        let layer = Id::new("modal");
        egui::Area::new(layer).order(Order::Foreground).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            dialog_frame().show(ui, |ui| {
                ui.set_width(440.0);
                let (title, message, accent) = match modal {
                    Modal::Message { title, message, error, .. } => (title, message, if *error { theme::red() } else { theme::accent() }),
                    Modal::Confirm { title, message, danger, .. } => (title, message, if *danger { theme::red() } else { theme::accent() }),
                };
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                    ui.painter().circle_filled(dot.center(), 5.0, accent);
                    ui.label(theme::title(title, 18.0));
                });
                ui.add_space(4.0);
                ui.label(RichText::new(message).color(theme::text()));
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match modal {
                    Modal::Message { .. } => {
                        if theme::filled_button(ui, "OK", theme::accent(), vec2(96.0, 36.0)).clicked() {
                            dismissed = true;
                        }
                    }
                    Modal::Confirm { confirm, danger, .. } => {
                        if theme::filled_button(ui, confirm, if *danger { theme::red() } else { theme::accent() }, vec2(120.0, 36.0)).clicked() {
                            confirmed = Some(());
                        }
                        if theme::quiet_button(ui, "Cancel").clicked() {
                            dismissed = true;
                        }
                    }
                });
            });
        });
        ctx.move_to_top(LayerId::new(Order::Foreground, layer));
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            dismissed = true;
        }

        if dismissed {
            if let Some(Modal::Message { close_app: true, .. }) = self.modal {
                self.allow_close = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            self.modal = None;
        } else if confirmed.is_some() {
            if let Some(Modal::Confirm { action, .. }) = self.modal.take() {
                match action {
                    ConfirmAction::Install => setup::install(self),
                    ConfirmAction::Uninstall => control::uninstall(self),
                    ConfirmAction::DiscardAndClose => {
                        self.allow_close = true;
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                }
            }
        }
    }

    fn show_overlays(&mut self, ctx: &egui::Context) {
        if let Some(job) = &self.job {
            dim_background(ctx, "job-dim");
            let layer = Id::new("job");
            egui::Area::new(layer).order(Order::Foreground).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
                dialog_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(22.0).color(theme::accent()));
                        ui.label(theme::title(&job.label, 16.0));
                    });
                });
            });
            ctx.move_to_top(LayerId::new(Order::Foreground, layer));
        }

        if let Some((message, since)) = &self.toast {
            let age = since.elapsed().as_secs_f32();
            if age > 3.5 {
                self.toast = None;
            } else {
                let alpha = ((3.5 - age) / 0.4).min(1.0);
                egui::Area::new(Id::new("toast"))
                    .order(Order::Tooltip)
                    .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -28.0))
                    .interactable(false)
                    .show(ctx, |ui| {
                        Frame::none()
                            .fill(theme::raised().linear_multiply(alpha))
                            .stroke(Stroke::new(1.0_f32, theme::accent().linear_multiply(alpha)))
                            .rounding(Rounding::same(10.0))
                            .inner_margin(Margin::symmetric(16.0, 10.0))
                            .show(ui, |ui| ui.label(RichText::new(message).color(theme::text().linear_multiply(alpha))));
                    });
                ctx.request_repaint();
            }
        }
    }
}

fn dialog_frame() -> Frame {
    Frame::none()
        .fill(theme::surface())
        .stroke(Stroke::new(1.0_f32, theme::border()))
        .rounding(Rounding::same(14.0))
        .inner_margin(Margin::same(22.0))
        .shadow(egui::epaint::Shadow { offset: vec2(0.0, 10.0), blur: 40.0, spread: 0.0, color: Color32::from_black_alpha(if theme::is_dark() { 140 } else { 45 }) })
}

fn dim_background(ctx: &egui::Context, id: &str) {
    egui::Area::new(Id::new(id)).order(Order::Foreground).fixed_pos(egui::Pos2::ZERO).show(ctx, |ui| {
        // Leaves the window bar usable, so the window can still be moved or closed.
        let mut screen = ctx.screen_rect();
        screen.min.y += WINDOW_BAR_HEIGHT;
        ui.allocate_rect(screen, Sense::click_and_drag());
        ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(if theme::is_dark() { 150 } else { 90 }));
    });
}

/// Whether this program is not the installed copy and differs from it (i.e. a newer download).
fn copy_differs_from_installed() -> bool {
    let Ok(current) = std::env::current_exe() else { return false };
    let installed = paths::installed_exe();
    if paths::same_path(&current.to_string_lossy(), &installed.to_string_lossy()) {
        return false;
    }
    match (std::fs::read(&current), std::fs::read(&installed)) {
        (Ok(a), Ok(b)) => a != b,
        _ => false,
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_secs(1));
        self.poll_job();
        self.handle_close_request(ctx);
        if self.screen == Screen::Control && self.last_reload.elapsed() > Duration::from_secs(5) {
            self.reload_saved();
        }

        let interactive = self.modal.is_none() && self.job.is_none() && !self.settings_open;
        window_bar(ctx);
        match self.screen {
            Screen::Setup => setup::show(self, ctx, interactive),
            Screen::Control => {
                control::show(self, ctx, interactive);
                settings::show(self, ctx);
            }
        }
        self.show_modal(ctx);
        self.show_overlays(ctx);
        self.persist_setup_progress(false);
        resize_edges(ctx);
    }
}

const WINDOW_BAR_HEIGHT: f32 = 34.0;

fn is_maximized(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false))
}

/// Makes empty space in `rect` move the window, and double-clicks maximise it. Call before adding
/// the widgets that sit on top, so they keep their own clicks.
pub(crate) fn drag_window(ui: &mut egui::Ui, rect: egui::Rect) {
    let response = ui.interact(rect, Id::new(("window-drag", rect.min.x as i32, rect.min.y as i32)), Sense::click_and_drag());
    if response.double_clicked() {
        let maximized = is_maximized(ui.ctx());
        ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    } else if response.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

/// Replaces the Windows title bar: a draggable strip with minimise, maximise and close buttons.
fn window_bar(ctx: &egui::Context) {
    egui::TopBottomPanel::top("window-bar")
        .frame(Frame::none().fill(theme::bg()))
        .show_separator_line(false)
        .exact_height(WINDOW_BAR_HEIGHT)
        .show(ctx, |ui| {
            drag_window(ui, ui.max_rect());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if window_button(ui, WindowButton::Close).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
                let maximized = is_maximized(ctx);
                let icon = if maximized { WindowButton::Restore } else { WindowButton::Maximize };
                if window_button(ui, icon).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                }
                if window_button(ui, WindowButton::Minimize).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                }
            });
        });
}

#[derive(Clone, Copy, PartialEq)]
enum WindowButton {
    Minimize,
    Maximize,
    Restore,
    Close,
}

fn window_button(ui: &mut egui::Ui, kind: WindowButton) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(46.0, WINDOW_BAR_HEIGHT), Sense::click());
    let close = kind == WindowButton::Close;
    let hovered = response.hovered();
    if hovered {
        let fill = if close { Color32::from_rgb(0xe8, 0x11, 0x23) } else { theme::raised() };
        let fill = if response.is_pointer_button_down_on() && !close { theme::pressed() } else { fill };
        ui.painter().rect_filled(rect, 0.0, fill);
    }
    let color = if close && hovered { Color32::WHITE } else if hovered { theme::text() } else { theme::muted_color() };
    let stroke = Stroke::new(1.0_f32, color);
    let c = rect.center();
    let painter = ui.painter();
    match kind {
        WindowButton::Minimize => {
            painter.line_segment([c + vec2(-5.0, 0.0), c + vec2(5.0, 0.0)], stroke);
        }
        WindowButton::Maximize => {
            painter.rect_stroke(egui::Rect::from_center_size(c, vec2(10.0, 10.0)), 1.5, stroke);
        }
        WindowButton::Restore => {
            painter.rect_stroke(egui::Rect::from_center_size(c + vec2(-1.5, 1.5), vec2(8.0, 8.0)), 1.5, stroke);
            painter.line_segment([c + vec2(-1.5, -4.5), c + vec2(4.5, -4.5)], stroke);
            painter.line_segment([c + vec2(4.5, -4.5), c + vec2(4.5, 1.5)], stroke);
        }
        WindowButton::Close => {
            painter.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], stroke);
            painter.line_segment([c + vec2(-5.0, 5.0), c + vec2(5.0, -5.0)], stroke);
        }
    }
    response
}

/// Without the Windows frame, the window edges have to start resizing themselves.
fn resize_edges(ctx: &egui::Context) {
    use egui::viewport::ResizeDirection as D;
    if is_maximized(ctx) {
        return;
    }
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let screen = ctx.screen_rect();
    let edge = 6.0;
    let (left, right) = (pos.x - screen.left() < edge, screen.right() - pos.x < edge);
    let (top, bottom) = (pos.y - screen.top() < edge, screen.bottom() - pos.y < edge);
    let direction = match (left, right, top, bottom) {
        (true, _, true, _) => D::NorthWest,
        (_, true, true, _) => D::NorthEast,
        (true, _, _, true) => D::SouthWest,
        (_, true, _, true) => D::SouthEast,
        (true, _, _, _) => D::West,
        (_, true, _, _) => D::East,
        (_, _, true, _) => D::North,
        (_, _, _, true) => D::South,
        _ => return,
    };
    ctx.set_cursor_icon(match direction {
        D::North | D::South => egui::CursorIcon::ResizeVertical,
        D::East | D::West => egui::CursorIcon::ResizeHorizontal,
        D::NorthWest | D::SouthEast => egui::CursorIcon::ResizeNwSe,
        D::NorthEast | D::SouthWest => egui::CursorIcon::ResizeNeSw,
    });
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
    }
}

pub(crate) fn header(ui: &mut egui::Ui, subtitle: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
        ui.painter().rect_filled(rect, 10.0, theme::accent());
        ui.painter().circle_stroke(rect.center(), 11.0, Stroke::new(3.0_f32, theme::on_accent()));
        let d = 7.5;
        ui.painter().line_segment(
            [rect.center() + vec2(-d, d), rect.center() + vec2(d, -d)],
            Stroke::new(3.0_f32, theme::on_accent()),
        );
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(theme::title(APP_NAME, 22.0));
            ui.label(RichText::new(subtitle).color(theme::muted_color()).size(13.5));
        });
    });
}
