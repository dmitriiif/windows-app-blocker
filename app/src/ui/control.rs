//! The control panel shown after setup.

use super::theme;
use super::timeline::{self, Schedule};
use super::{App, ConfirmAction, JobDone, Modal};
use crate::config::{self, Config};
use crate::install;
use crate::paths::{self, APP_NAME};
use crate::policy::{can_use, unavailable_message};
use crate::schedule::{is_blocked, next_change};
use crate::task;
use chrono::{Local, NaiveDateTime};
use egui::{vec2, Align, Color32, Frame, Layout, Margin, RichText, Rounding, Sense, Stroke, Ui};
use std::path::Path;

pub(super) fn show(app: &mut App, ctx: &egui::Context, interactive: bool) {
    let now = Local::now().naive_local();
    let Some(saved) = app.saved.clone() else { return };

    egui::TopBottomPanel::top("control-header")
        .frame(Frame::none().fill(theme::bg()).inner_margin(Margin { left: 28.0, right: 28.0, top: 6.0, bottom: 8.0 }))
        .show_separator_line(false)
        .show(ctx, |ui| {
            super::drag_window(ui, ui.max_rect());
            ui.add_enabled_ui(interactive, |ui| {
                ui.horizontal(|ui| {
                    super::header(ui, "Block selected Windows apps on your schedule");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        protection_pill(app, ui, &saved, now);
                        ui.add_space(4.0);
                        if theme::quiet_button(ui, "⚙  Settings").clicked() {
                            app.settings_open = true;
                        }
                    });
                });
                if app.update_available {
                    ui.add_space(6.0);
                    banner(ui, theme::accent(), "This copy of the app is different from the installed one.", |ui| {
                        if theme::filled_button(ui, "Update installed app", vec2(0.0, 30.0)).clicked() {
                            let config = saved.clone();
                            app.start_job("Updating the installed app…", move || install::install(&config).map(|_| JobDone::Updated));
                        }
                    });
                }
            });
        });

    egui::TopBottomPanel::bottom("control-footer")
        .frame(Frame::none().fill(theme::surface()).stroke(Stroke::new(1.0_f32, theme::border())).inner_margin(Margin::symmetric(28.0, 14.0)))
        .show(ctx, |ui| {
            ui.add_enabled_ui(interactive, |ui| footer(app, ui, &saved, now));
        });

    egui::CentralPanel::default().frame(Frame::none().fill(theme::bg()).inner_margin(Margin::symmetric(28.0, 12.0))).show(ctx, |ui| {
        ui.add_enabled_ui(interactive, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                theme::card(ui, |ui| {
                    theme::section_title(ui, "Apps to block", Some("Only these exact programs are closed, and only for your Windows account."));
                    let can_remove = can_use(saved.policies.remove_executables, &saved, now);
                    let reason = unavailable_message(saved.policies.remove_executables, "Apps cannot be removed");
                    apps_editor(ui, app, can_remove, &saved.executables, &reason);
                });
                ui.add_space(14.0);
                theme::card(ui, |ui| {
                    let editable = can_use(saved.policies.change_times, &saved, now);
                    ui.horizontal(|ui| {
                        theme::section_title(ui, "Blocking schedule", None);
                        if !editable {
                            theme::badge(ui, "LOCKED", theme::amber())
                                .on_hover_text(unavailable_message(saved.policies.change_times, "Blocking hours cannot be changed"));
                        }
                    });
                    if !editable {
                        ui.label(RichText::new(unavailable_message(saved.policies.change_times, "Blocking hours cannot be changed")).color(theme::amber()));
                    }
                    let draft = &mut app.draft;
                    let schedule = Schedule { mode: &mut draft.schedule_mode, days: &mut draft.days };
                    if timeline::show(ui, &mut app.timeline, schedule, editable, now) {
                        app.timeline.error = None;
                    }
                });
                ui.add_space(8.0);
            });
        });
    });
}

fn banner(ui: &mut Ui, color: Color32, text: &str, add: impl FnOnce(&mut Ui)) {
    Frame::none()
        .fill(color.linear_multiply(0.12))
        .stroke(Stroke::new(1.0_f32, color.linear_multiply(0.5)))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::symmetric(14.0, 8.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(text).color(theme::text()));
                ui.with_layout(Layout::right_to_left(Align::Center), add);
            });
        });
}

fn count_text(count: usize) -> String {
    format!("{count} {}", if count == 1 { "app is" } else { "apps are" })
}

fn describe_change(now: NaiveDateTime, at: NaiveDateTime, blocked: bool) -> String {
    let minutes = (at - now).num_minutes().max(0) as u32 + 1;
    let when = if at.date() == now.date() {
        at.format("%H:%M").to_string()
    } else if at.date() == now.date().succ_opt().unwrap_or(now.date()) {
        at.format("tomorrow %H:%M").to_string()
    } else {
        at.format("%A %H:%M").to_string()
    };
    let what = if blocked { "Apps allowed again" } else { "Next block starts" };
    format!("{what} {when} · in {}", crate::time::format_duration(minutes))
}

/// Compact status pill with the protection on/off switch, shown in the header.
fn protection_pill(app: &mut App, ui: &mut Ui, saved: &Config, now: NaiveDateTime) {
    let task_state = app.task.get();
    let blocked = is_blocked(saved, now);
    let count = saved.executables.len();

    enum State {
        Checking,
        Missing,
        On,
        Off,
        Attention(String),
    }
    let state = match (&app.load_error, &task_state) {
        (Some(error), _) => State::Attention(error.clone()),
        (None, None) => State::Checking,
        (None, Some(None)) => State::Missing,
        (None, Some(Some(info))) if info.enabled => State::On,
        (None, Some(Some(_))) => State::Off,
    };
    let (color, title, detail) = match &state {
        State::Checking => (theme::muted_color(), "Checking…", "Reading the background task.".to_owned()),
        State::Missing => (theme::amber(), "Needs attention", format!("The background task is missing. Repair {APP_NAME} to restore it.")),
        State::Attention(error) => (theme::amber(), "Needs attention", error.clone()),
        State::On if blocked => (theme::green(), "Blocking now", format!("{} blocked right now.", count_text(count))),
        State::On => (theme::green(), "Protection on", format!("{} currently allowed; the schedule is still running.", count_text(count))),
        State::Off => (theme::red(), "Protection off", format!("{} configured; all apps are currently allowed.", count_text(count))),
    };

    // Measure the text first: in this right-to-left row, a plain child ui would stretch across the whole header.
    let title_font = egui::FontId::new(15.0, theme::semibold());
    let change_font = egui::FontId::proportional(12.0);
    let change = next_change(saved, now).map(|at| describe_change(now, at, blocked));
    let text_size = ui.fonts(|f| {
        let t = f.layout_no_wrap(title.to_owned(), title_font.clone(), color).size();
        let c = change.as_ref().map_or(vec2(0.0, 0.0), |c| f.layout_no_wrap(c.clone(), change_font.clone(), theme::muted_color()).size());
        vec2(t.x.max(c.x) + 1.0, t.y + c.y + 1.0)
    });
    let row_height = text_size.y.max(28.0);
    let centered = Layout::centered_and_justified(egui::Direction::TopDown);

    // Called inside a right-to-left layout, so widgets are added from the right edge inward.
    Frame::none()
        .fill(theme::raised())
        .stroke(Stroke::new(1.0_f32, color.linear_multiply(0.35)))
        .rounding(Rounding::same(999.0))
        .inner_margin(Margin { left: 16.0, right: 10.0, top: 8.0, bottom: 8.0 })
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            match state {
                State::On | State::Off => {
                    let was_on = matches!(state, State::On);
                    let allowed = !was_on || can_use(saved.policies.turn_off, saved, now);
                    let mut on = was_on;
                    let response = ui
                        .allocate_ui_with_layout(vec2(52.0, row_height), centered, |ui| {
                            ui.add_enabled_ui(allowed, |ui| theme::toggle_colored(ui, &mut on, vec2(52.0, 28.0), theme::green())).inner
                        })
                        .inner
                        .on_hover_text(if was_on { "Turn protection off" } else { "Turn protection on" })
                        .on_disabled_hover_text(unavailable_message(saved.policies.turn_off, "Turning protection off is unavailable"));
                    if response.clicked() {
                        toggle_protection(app, was_on);
                    }
                }
                State::Missing => {
                    let clicked = ui
                        .allocate_ui_with_layout(vec2(90.0, row_height), centered, |ui| theme::filled_button(ui, "Repair", vec2(90.0, 30.0)).clicked())
                        .inner;
                    if clicked {
                        let config = saved.clone();
                        app.start_job("Repairing…", move || install::install(&config).map(|_| JobDone::Updated));
                    }
                }
                _ => {}
            }
            ui.allocate_ui_with_layout(vec2(text_size.x, row_height), Layout::top_down(Align::Min).with_main_align(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(RichText::new(title).font(title_font).color(color));
                if let Some(change) = change {
                    ui.label(RichText::new(change).font(change_font).color(theme::muted_color()));
                }
            });
            let (dot, _) = ui.allocate_ui_with_layout(vec2(14.0, row_height), centered, |ui| ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover())).inner;
            let pulse = if matches!(state, State::On) { (ui.input(|i| i.time) * 2.0).sin() as f32 * 0.5 + 0.5 } else { 0.0 };
            ui.painter().circle_filled(dot.center(), 6.0 + pulse * 2.0, color.linear_multiply(0.25));
            ui.painter().circle_filled(dot.center(), 4.5, color);
        })
        .response
        .on_hover_text(detail);
}

/// The list of blocked apps with add/remove. Apps in `protected` can only be removed when
/// `can_remove_protected` is true; apps added since are always removable.
pub(super) fn apps_editor(ui: &mut Ui, app: &mut App, can_remove_protected: bool, protected: &[String], locked_reason: &str) {
    if app.draft.executables.is_empty() {
        Frame::none()
            .fill(theme::row())
            .stroke(Stroke::new(1.0_f32, theme::border()))
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::same(18.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| ui.label(theme::muted("No apps yet. Add the .exe files you want to block.")));
            });
    }

    let mut toggled: Option<String> = None;
    for path in &app.draft.executables {
        let selected = app.selected_apps.iter().any(|s| s == path);
        let name = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
        let frame = Frame::none()
            .fill(if selected { theme::row_selected() } else { theme::row() })
            .stroke(Stroke::new(1.0_f32, if selected { theme::accent().linear_multiply(0.6) } else { theme::border() }))
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::symmetric(12.0, 8.0));
        let row = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                theme::app_avatar(ui, &name);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(RichText::new(&name).font(egui::FontId::new(14.5, theme::semibold())));
                    ui.add(egui::Label::new(theme::muted(path.as_str()).size(12.0)).truncate());
                });
                let versioned = path.contains('*');
                if paths::expand(path).is_empty() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        theme::badge(ui, "NOT FOUND", theme::amber()).on_hover_text("This file does not exist right now. It will still be blocked if it appears.");
                    });
                } else if versioned {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        theme::badge(ui, "ALL VERSIONS", theme::accent()).on_hover_text("Every version of this app is blocked, so updates do not undo the block.");
                    });
                }
            });
        });
        let response = ui.interact(row.response.rect, egui::Id::new(("app-row", path)), Sense::click());
        if response.clicked() {
            toggled = Some(path.clone());
        }
    }
    if let Some(path) = toggled {
        if let Some(i) = app.selected_apps.iter().position(|s| *s == path) {
            app.selected_apps.remove(i);
        } else {
            app.selected_apps.push(path);
        }
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if theme::filled_button(ui, "+  Add .exe…", vec2(0.0, 34.0)).clicked() {
            add_executables(app);
        }
        if ui.add(egui::Button::new("Find common apps…").fill(theme::raised()).rounding(Rounding::same(8.0)).min_size(vec2(0.0, 34.0)))
            .on_hover_text("Look for installed browsers, game stores and chat apps such as Chrome, Steam and Discord.")
            .clicked()
        {
            super::finder::open(app);
        }
        let selection: Vec<String> = app.selected_apps.clone();
        let blocked_by_policy = !can_remove_protected && selection.iter().any(|s| protected.iter().any(|p| paths::same_path(p, s)));
        let enabled = !selection.is_empty() && !blocked_by_policy;
        let label = match selection.len() {
            0 | 1 => "Remove selected".to_owned(),
            n => format!("Remove {n} selected"),
        };
        let response = ui.add_enabled(enabled, egui::Button::new(label).fill(theme::raised()).rounding(Rounding::same(8.0)).min_size(vec2(0.0, 34.0)));
        let response = if blocked_by_policy {
            response.on_disabled_hover_text(locked_reason)
        } else {
            response.on_disabled_hover_text("Click apps in the list to select them.")
        };
        if response.clicked() {
            app.draft.executables.retain(|e| !selection.contains(e));
            app.selected_apps.clear();
        }
        if blocked_by_policy {
            ui.label(RichText::new(locked_reason).color(theme::amber()).size(12.5));
        } else if selection.is_empty() && !app.draft.executables.is_empty() {
            ui.label(theme::muted("Click an app to select it.").size(12.5));
        }
    });
}

fn add_executables(app: &mut App) {
    let Some(files) = rfd::FileDialog::new().set_title("Choose apps to block").add_filter("Windows applications", &["exe"]).pick_files() else {
        return;
    };
    for file in files {
        let full = std::path::absolute(&file).unwrap_or(file).to_string_lossy().into_owned();
        if config::validate_executable(&full).is_err() {
            continue;
        }
        if !app.draft.executables.iter().any(|e| paths::same_path(e, &full)) {
            app.draft.executables.push(full);
        }
    }
}

/// Saves the app list, and the schedule when changing hours is allowed right now.
fn save_changes(app: &mut App, announce: bool) -> Result<(), String> {
    let saved = app.saved.clone().ok_or("The configuration is not loaded.")?;
    let now = Local::now().naive_local();
    let can_change_times = can_use(saved.policies.change_times, &saved, now);
    let mut next = saved.clone();
    next.executables = app.draft.executables.clone();
    if can_change_times {
        app.draft.validate_schedule()?;
        next.schedule_mode = app.draft.schedule_mode;
        next.days = app.draft.days.clone();
    }
    config::save(&next, &paths::config_path())?;
    app.saved = Some(next.clone());
    app.draft = next;
    if announce {
        app.toast(if can_change_times {
            "Your apps and blocking hours are saved."
        } else {
            "Your app list is saved. Blocking hours are locked right now."
        });
    }
    Ok(())
}

fn toggle_protection(app: &mut App, currently_on: bool) {
    if let Err(message) = save_changes(app, false) {
        app.error("Could not change protection", message);
        return;
    }
    let Some(saved) = app.saved.clone() else { return };
    let now = Local::now().naive_local();
    if currently_on {
        if !can_use(saved.policies.turn_off, &saved, now) {
            app.error("Could not change protection", unavailable_message(saved.policies.turn_off, "Turning protection off is unavailable"));
            return;
        }
        app.start_job("Turning protection off…", || {
            task::stop_and_disable()?;
            Ok(JobDone::Protection(task::query()))
        });
    } else {
        if saved.executables.is_empty() {
            app.error("Could not change protection", "Add at least one .exe before turning protection on.");
            return;
        }
        app.start_job("Turning protection on…", || {
            if task::query().is_none() {
                return Err(format!("The background task is missing. Reinstall {APP_NAME}."));
            }
            task::enable_and_run()?;
            Ok(JobDone::Protection(task::query()))
        });
    }
}

pub(super) fn uninstall(app: &mut App) {
    app.start_job("Uninstalling…", || install::uninstall().map(|_| JobDone::Uninstalled));
}

fn footer(app: &mut App, ui: &mut Ui, saved: &Config, now: NaiveDateTime) {
    let dirty = app.is_dirty();
    ui.horizontal(|ui| {
        let save = ui.add_enabled_ui(dirty, |ui| theme::filled_button(ui, "Save changes", vec2(150.0, 38.0))).inner;
        if save.clicked() {
            if let Err(message) = save_changes(app, true) {
                app.error("Could not save changes", message);
            }
        }
        if dirty {
            ui.label(RichText::new("Unsaved changes").color(theme::amber()));
            if ui.add(egui::Button::new(theme::muted("Undo")).fill(Color32::TRANSPARENT)).clicked() {
                app.draft = saved.clone();
                app.selected_apps.clear();
                app.timeline.error = None;
            }
        } else {
            ui.label(theme::muted("Protection keeps running when this window closes."));
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let can_uninstall = can_use(saved.policies.uninstall, saved, now);
            let response = ui
                .add_enabled(can_uninstall, egui::Button::new(RichText::new("Uninstall").color(theme::muted_color())).fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0_f32, theme::border())))
                .on_hover_text(format!("Remove {APP_NAME} from this computer. Your settings are kept for a reinstall."))
                .on_disabled_hover_text(unavailable_message(saved.policies.uninstall, "Uninstall is unavailable"));
            if response.clicked() {
                app.modal = Some(Modal::Confirm {
                    title: format!("Uninstall {APP_NAME}?"),
                    message: format!(
                        "This will turn protection off and remove {APP_NAME} from this computer.\n\nYour apps, schedule and lock choices are kept and will be restored if you install it again.",
                    ),
                    confirm: "Uninstall".into(),
                    danger: true,
                    action: ConfirmAction::Uninstall,
                });
            }
        });
    });
}
