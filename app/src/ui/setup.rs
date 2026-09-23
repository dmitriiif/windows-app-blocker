//! First-run (and reinstall) setup wizard.

use super::control::apps_editor;
use super::theme;
use super::timeline::{self, Schedule};
use super::{App, ConfirmAction, JobDone, Modal};
use crate::config::{is_valid_sid, Policy, ScheduleMode};
use crate::install;
use crate::policy::{can_use, choice_label, display, unavailable_message};
use crate::win;
use chrono::Local;
use egui::{vec2, Align, Color32, Frame, Layout, Margin, RichText, Rounding, Sense, Stroke, Ui};

const STEPS: [&str; 4] = ["Lock choices", "Apps", "Schedule", "Finish"];
const LOCKS: usize = 0;
const APPS: usize = 1;
const SCHEDULE: usize = 2;

pub(super) fn show(app: &mut App, ctx: &egui::Context, interactive: bool) {
    egui::TopBottomPanel::top("setup-header")
        .frame(Frame::none().fill(theme::bg()).inner_margin(Margin { left: 28.0, right: 28.0, top: 0.0, bottom: 10.0 }))
        .show_separator_line(false)
        .show(ctx, |ui| {
            super::drag_window(ui, ui.max_rect());
            super::header(ui, "Setup — choose which controls stay available, the apps and the schedule");
            ui.add_space(10.0);
            stepper(ui, app.setup.step);
        });

    egui::TopBottomPanel::bottom("setup-footer")
        .frame(Frame::none().fill(theme::surface()).stroke(Stroke::new(1.0_f32, theme::border())).inner_margin(Margin::symmetric(28.0, 14.0)))
        .show(ctx, |ui| {
            ui.add_enabled_ui(interactive, |ui| footer(app, ui));
        });

    egui::CentralPanel::default().frame(Frame::none().fill(theme::bg()).inner_margin(Margin::symmetric(28.0, 10.0))).show(ctx, |ui| {
        ui.add_enabled_ui(interactive, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if let Some(notice) = &app.setup.notice {
                    Frame::none()
                        .fill(theme::accent().linear_multiply(0.12))
                        .stroke(Stroke::new(1.0_f32, theme::accent().linear_multiply(0.45)))
                        .rounding(Rounding::same(10.0))
                        .inner_margin(Margin::symmetric(14.0, 10.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(notice).color(theme::text()));
                        });
                    ui.add_space(10.0);
                }
                match app.setup.step {
                    LOCKS => policies_step(app, ui),
                    APPS => apps_step(app, ui),
                    SCHEDULE => schedule_step(app, ui),
                    _ => finish_step(app, ui),
                }
            });
        });
    });
}

fn stepper(ui: &mut Ui, current: usize) {
    ui.horizontal(|ui| {
        for (index, name) in STEPS.iter().enumerate() {
            let done = index < current;
            let active = index == current;
            let (circle, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
            let color = if active { theme::accent() } else if done { theme::green() } else { theme::raised() };
            ui.painter().circle_filled(circle.center(), 13.0, color);
            if done {
                let c = circle.center();
                let check = vec![c + vec2(-5.0, 0.5), c + vec2(-1.5, 4.0), c + vec2(5.0, -3.5)];
                ui.painter().add(egui::Shape::line(check, Stroke::new(2.2_f32, Color32::WHITE)));
            } else {
                ui.painter().text(
                    circle.center(),
                    egui::Align2::CENTER_CENTER,
                    (index + 1).to_string(),
                    egui::FontId::new(13.0, theme::semibold()),
                    if active { theme::on_accent() } else { theme::muted_color() },
                );
            }
            ui.label(RichText::new(*name).color(if active { theme::text() } else { theme::muted_color() }).font(egui::FontId::new(
                14.0,
                if active { theme::semibold() } else { egui::FontFamily::Proportional },
            )));
            if index + 1 < STEPS.len() {
                let (line, _) = ui.allocate_exact_size(vec2(36.0, 26.0), Sense::hover());
                ui.painter().line_segment([line.left_center(), line.right_center()], Stroke::new(2.0_f32, if done { theme::green() } else { theme::border() }));
            }
        }
    });
}

/// Whether the schedule may be edited during this setup.
fn schedule_editable(app: &App) -> bool {
    match &app.setup.previous {
        Some(previous) if app.setup.locked_policies => can_use(previous.policies.change_times, previous, Local::now().naive_local()),
        _ => true,
    }
}

/// Whether a step may be skipped. The first step never can, and the schedule cannot when it
/// could never be changed afterwards.
fn can_skip(app: &App, step: usize) -> bool {
    match step {
        APPS => true,
        SCHEDULE => !(schedule_editable(app) && app.draft.policies.change_times == Policy::Never),
        _ => false,
    }
}

/// A tinted note inside a step card.
fn note(ui: &mut Ui, color: Color32, text: &str) {
    Frame::none()
        .fill(color.linear_multiply(0.10))
        .stroke(Stroke::new(1.0_f32, color.linear_multiply(0.40)))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::symmetric(14.0, 9.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(text).color(theme::text()));
        });
    ui.add_space(4.0);
}

fn schedule_step(app: &mut App, ui: &mut Ui) {
    let editable = schedule_editable(app);
    theme::card(ui, |ui| {
        theme::section_title(ui, "3. When should apps be blocked?", Some("Add as many blocks as you like to each lane. Overnight blocks are supported."));
        if !editable {
            if let Some(previous) = &app.setup.previous {
                ui.label(RichText::new(unavailable_message(previous.policies.change_times, "Blocking hours cannot be changed")).color(theme::amber()));
            }
        } else {
            match app.draft.policies.change_times {
                Policy::Always => note(ui, theme::accent(), "Optional — you can skip this step and keep the hours shown. Blocking hours can be changed any time later from the control panel."),
                Policy::AllowedHoursOnly => note(ui, theme::accent(), "Optional — you can skip this step and keep the hours shown. Blocking hours can be changed later from the control panel, outside blocking hours."),
                Policy::Never => note(ui, theme::amber(), "You chose never to change blocking hours after setup, so this step cannot be skipped. Set your hours carefully now."),
            }
        }
        let draft = &mut app.draft;
        let schedule = Schedule { mode: &mut draft.schedule_mode, days: &mut draft.days };
        timeline::show(ui, &mut app.timeline, schedule, editable, Local::now().naive_local());
    });
}

fn policies_step(app: &mut App, ui: &mut Ui) {
    let locked = app.setup.locked_policies;
    theme::card(ui, |ui| {
        theme::section_title(
            ui,
            "1. Choose what can be changed later",
            Some("“Only during allowed hours” means only while the selected apps are not scheduled to be blocked."),
        );
        if locked {
            ui.horizontal(|ui| {
                theme::badge(ui, "LOCKED", theme::amber());
                ui.label(RichText::new("Kept from your previous installation. These choices cannot be changed.").color(theme::amber()));
            });
            ui.add_space(4.0);
        }
        let options = [
            (Policy::Always, choice_label(Policy::Always)),
            (Policy::Never, choice_label(Policy::Never)),
            (Policy::AllowedHoursOnly, choice_label(Policy::AllowedHoursOnly)),
        ];
        let policies = &mut app.draft.policies;
        let rows: [(&str, &str, &mut Policy); 4] = [
            ("Change blocking hours", "Edit the timeline after setup.", &mut policies.change_times),
            ("Turn protection off", "Turning protection on is always allowed.", &mut policies.turn_off),
            ("Uninstall this app", "Your settings are kept either way.", &mut policies.uninstall),
            ("Remove apps from the block list", "Adding apps is always allowed.", &mut policies.remove_executables),
        ];
        egui::Grid::new("policies").num_columns(2).spacing(vec2(24.0, 14.0)).show(ui, |ui| {
            for (label, hint, value) in rows {
                ui.vertical(|ui| {
                    ui.set_min_width(290.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(RichText::new(label).font(egui::FontId::new(14.5, theme::semibold())));
                    ui.label(theme::muted(hint).size(12.5));
                });
                ui.add_enabled_ui(!locked, |ui| theme::segmented(ui, value, &options));
                ui.end_row();
            }
        });
    });
}

fn apps_step(app: &mut App, ui: &mut Ui) {
    let now = Local::now().naive_local();
    let (can_remove, protected, reason) = match (&app.setup.previous, app.setup.locked_policies) {
        (Some(previous), true) => (
            can_use(previous.policies.remove_executables, previous, now),
            previous.executables.clone(),
            unavailable_message(previous.policies.remove_executables, "Apps from your previous installation cannot be removed"),
        ),
        _ => (true, Vec::new(), String::new()),
    };
    let removal = match app.draft.policies.remove_executables {
        Policy::Always => "removed any time".to_owned(),
        other => format!("removed {}", display(other)),
    };
    let later = format!("Optional — you can skip this step. Apps can be added any time later from the control panel, and {removal}.");
    theme::card(ui, |ui| {
        theme::section_title(ui, "2. Choose apps to block", Some("Select the program files (.exe). Only these exact files are closed."));
        note(ui, theme::accent(), &later);
        apps_editor(ui, app, can_remove, &protected, &reason);
    });
}

fn finish_step(app: &mut App, ui: &mut Ui) {
    theme::card(ui, |ui| {
        theme::section_title(ui, "4. Create shortcuts", None);
        ui.checkbox(&mut app.draft.shortcuts.start_menu, "Add to Start menu");
        ui.checkbox(&mut app.draft.shortcuts.desktop, "Add to desktop");
    });
    ui.add_space(14.0);
    theme::card(ui, |ui| {
        theme::section_title(ui, "Summary", None);
        let c = &app.draft;
        let mode = match c.schedule_mode {
            ScheduleMode::EveryDay => "Same hours every day",
            ScheduleMode::WeekdayWeekend => "Separate weekday and weekend hours",
            ScheduleMode::IndividualDays => "Individual hours for each day",
        };
        let blocks: usize = crate::config::Track::lanes(c.schedule_mode).iter().map(|t| c.days.get(*t).blocks.len()).sum();
        let p = &c.policies;
        egui::Grid::new("summary").num_columns(2).spacing(vec2(24.0, 8.0)).show(ui, |ui| {
            for (label, value) in [
                ("Schedule", format!("{mode} · {blocks} block{}", if blocks == 1 { "" } else { "s" })),
                ("Apps", match c.executables.len() {
                    0 => "None yet — add them from the control panel".to_owned(),
                    n => format!("{n} selected"),
                }),
                ("Change hours", choice_label(p.change_times).to_owned()),
                ("Turn protection off", choice_label(p.turn_off).to_owned()),
                ("Uninstall", choice_label(p.uninstall).to_owned()),
                ("Remove apps", choice_label(p.remove_executables).to_owned()),
            ] {
                ui.label(theme::muted(label));
                ui.label(value);
                ui.end_row();
            }
        });
        ui.add_space(6.0);
        ui.label(RichText::new("Protection starts off, so setup cannot unexpectedly close an app. Turn it on from the control panel.").color(theme::faint()).size(12.5));
        if c.executables.is_empty() {
            ui.label(RichText::new("Add at least one app from the control panel before turning protection on.").color(theme::amber()).size(12.5));
        }
    });
}

fn validate_step(app: &App, step: usize) -> Result<(), String> {
    match step {
        SCHEDULE => app.draft.validate_schedule(),
        _ => Ok(()),
    }
}

fn footer(app: &mut App, ui: &mut Ui) {
    ui.horizontal(|ui| {
        let step = app.setup.step;
        if step > 0 && theme::quiet_button(ui, "Back").clicked() {
            app.setup.step -= 1;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if step + 1 < STEPS.len() {
                if theme::filled_button(ui, "Next", theme::accent(), vec2(130.0, 40.0)).clicked() {
                    match validate_step(app, step) {
                        Ok(()) => app.setup.step += 1,
                        Err(message) => app.error("Please check this step", message),
                    }
                }
                if can_skip(app, step) {
                    let skip = ui.add(egui::Button::new(RichText::new("Skip").color(theme::muted_color())).fill(Color32::TRANSPARENT).min_size(vec2(90.0, 40.0)));
                    if skip.on_hover_text("Continue without changing this step. You can change it later.").clicked() {
                        app.setup.step += 1;
                    }
                }
            } else if theme::filled_button(ui, "Install and finish setup", theme::green(), vec2(240.0, 44.0)).clicked() {
                request_install(app);
            }
        });
    });
}

fn request_install(app: &mut App) {
    for step in 0..STEPS.len() {
        if let Err(message) = validate_step(app, step) {
            app.setup.step = step;
            app.error("Please check this step", message);
            return;
        }
    }
    let p = &app.draft.policies;
    let locked: Vec<&str> = [
        (p.change_times, "change blocking hours"),
        (p.turn_off, "turn protection off"),
        (p.uninstall, "uninstall from this app"),
        (p.remove_executables, "remove apps from the block list"),
    ]
    .into_iter()
    .filter(|(policy, _)| *policy == Policy::Never)
    .map(|(_, name)| name)
    .collect();
    if locked.is_empty() || app.setup.locked_policies {
        install(app);
        return;
    }
    app.modal = Some(Modal::Confirm {
        title: "Confirm locked controls".into(),
        message: format!(
            "You chose ‘No — never’ for:\n\n• {}\n\nThese controls will stay disabled after setup, and they are kept if you reinstall. Windows administrators can still bypass this self-control tool outside the app. Continue?",
            locked.join("\n• ")
        ),
        confirm: "Continue".into(),
        danger: true,
        action: ConfirmAction::Install,
    });
}

/// Starts the installation with the wizard's choices.
pub(super) fn install(app: &mut App) {
    let mut config = app.draft.clone();
    if let Some(previous) = &app.setup.previous {
        if app.setup.locked_policies {
            config.policies = previous.policies;
            if !schedule_editable(app) {
                config.schedule_mode = previous.schedule_mode;
                config.days = previous.days.clone();
            }
        }
    }
    if !is_valid_sid(&config.target_user_sid) {
        match win::current_user_sid() {
            Some(sid) => config.target_user_sid = sid,
            None => {
                app.error("Could not install", "Could not identify your Windows account.");
                return;
            }
        }
    }
    config.setup_completed = true;
    app.start_job("Installing…", move || install::install(&config).map(|_| JobDone::Installed));
}
