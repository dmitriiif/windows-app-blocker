//! The settings dialog: colour theme and the heads-up notification before blocks.

use super::theme;
use super::{dialog_frame, dim_background, App, ConfirmAction, JobDone, Modal};
use crate::config::{self, Policies, Policy, Theme};
use crate::notify;
use crate::paths;
use crate::policy::{self, can_use, choice_label, unavailable_message};
use chrono::Local;
use egui::{pos2, vec2, Align, Align2, FontId, Id, Layout, LayerId, Order, Rect, RichText, Rounding, Sense, Stroke, Ui};

pub(super) fn show(app: &mut App, ctx: &egui::Context) {
    if !app.settings_open {
        return;
    }
    let Some(saved) = app.saved.clone() else { return };
    let mut prefs = saved.preferences;
    let mut locks = saved.policies;
    let locks_editable = can_use(saved.policies.change_locks, &saved, Local::now().naive_local());
    let mut close = false;
    let mut test = false;

    dim_background(ctx, "settings-dim");
    let layer = Id::new("settings");
    egui::Area::new(layer).order(Order::Foreground).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
        dialog_frame().show(ui, |ui| {
            ui.set_width(600.0);
            ui.horizontal(|ui| {
                ui.label(theme::title("Settings", 20.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if theme::quiet_button(ui, "Close").clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(6.0);
            let height = (ctx.screen_rect().height() - 200.0).max(200.0);
            egui::ScrollArea::vertical().max_height(height).auto_shrink([false, true]).show(ui, |ui| {
                theme::section_title(ui, "Theme", None);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    for option in Theme::ALL {
                        if theme_swatch(ui, option, prefs.theme == option).clicked() {
                            prefs.theme = option;
                        }
                    }
                });
                ui.add_space(12.0);

                theme::section_title(ui, "Heads-up before blocking", Some("A Windows notification shortly before a block starts, while protection is on."));
                ui.horizontal(|ui| {
                    theme::toggle(ui, &mut prefs.notify_before_block);
                    ui.label("Notify me before a block starts");
                });
                ui.add_enabled_ui(prefs.notify_before_block, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(theme::muted("How early"));
                        theme::segmented(ui, &mut prefs.notify_minutes, &[(5, "5 min"), (10, "10 min"), (15, "15 min")]);
                    });
                });
                ui.add_space(2.0);
                if theme::quiet_button(ui, "Send a test notification").clicked() {
                    test = true;
                }
                ui.add_space(12.0);
                lock_choices(ui, &mut locks, locks_editable, saved.policies.change_locks);
            });
        });
    });
    ctx.move_to_top(LayerId::new(Order::Foreground, layer));
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }

    if prefs != saved.preferences {
        let mut next = saved.clone();
        next.preferences = prefs;
        match config::save(&next, &paths::config_path()) {
            Ok(()) => {
                if prefs.theme != saved.preferences.theme {
                    theme::apply(ctx, prefs.theme);
                }
                app.saved = Some(next);
                app.draft.preferences = prefs;
            }
            Err(message) => app.error("Could not save settings", message),
        }
    }
    if locks != saved.policies {
        let newly_never: Vec<&str> = {
            let (mut old, mut new) = (saved.policies, locks);
            policy::rows(&mut old)
                .into_iter()
                .zip(policy::rows(&mut new))
                .filter(|((_, _, before), (_, _, after))| **after == Policy::Never && **before != Policy::Never)
                .map(|((label, _, _), _)| label)
                .collect()
        };
        if newly_never.is_empty() {
            save_locks(app, locks);
        } else {
            app.modal = Some(Modal::Confirm {
                title: "Lock this for good?".into(),
                message: format!(
                    "You chose ‘No — never’ for:

• {}

This cannot be undone from the app, even by reinstalling it. Continue?",
                    newly_never.join("
• ")
                ),
                confirm: "Lock it".into(),
                danger: true,
                action: ConfirmAction::SetLocks(locks),
            });
        }
    }
    if test {
        app.start_job("Sending a test notification…", || notify::send_test().map(|_| JobDone::TestNotification));
    }
    if close {
        app.settings_open = false;
    }
}

/// The lock choices made during setup: editable only when "Change these lock choices" allows it.
fn lock_choices(ui: &mut Ui, locks: &mut Policies, editable: bool, change_locks: Policy) {
    ui.horizontal(|ui| {
        theme::section_title(ui, "Lock choices", Some("Chosen during setup. They decide which controls stay available."));
        if !editable {
            theme::badge(ui, "LOCKED", theme::amber());
        }
    });
    if !editable {
        ui.label(RichText::new(unavailable_message(change_locks, "These cannot be changed")).color(theme::amber()).size(12.5));
    }
    ui.add_space(4.0);
    let options = policy::options();
    egui::Grid::new("settings-locks").num_columns(2).spacing(vec2(18.0, 10.0)).show(ui, |ui| {
        for (label, _, value) in policy::rows(locks) {
            ui.label(RichText::new(label).font(FontId::new(14.0, theme::semibold())));
            if editable {
                theme::segmented(ui, value, &options);
            } else {
                ui.label(theme::muted(choice_label(*value)));
            }
            ui.end_row();
        }
    });
}

/// Saves new lock choices, keeping everything else as saved.
pub(super) fn save_locks(app: &mut App, locks: Policies) {
    let Some(saved) = app.saved.clone() else { return };
    if !can_use(saved.policies.change_locks, &saved, Local::now().naive_local()) {
        app.error("Could not change lock choices", unavailable_message(saved.policies.change_locks, "Lock choices cannot be changed"));
        return;
    }
    let mut next = saved;
    next.policies = locks;
    match config::save(&next, &paths::config_path()) {
        Ok(()) => {
            app.draft.policies = locks;
            app.saved = Some(next);
            app.toast("Lock choices saved.");
        }
        Err(message) => app.error("Could not save lock choices", message),
    }
}

/// A miniature of the theme: its background, a card and an accent bar.
fn theme_swatch(ui: &mut Ui, option: Theme, selected: bool) -> egui::Response {
    let p = theme::palette(option);
    let (rect, response) = ui.allocate_exact_size(vec2(100.0, 92.0), Sense::click());
    let preview = Rect::from_min_size(rect.min, vec2(rect.width(), 64.0));
    let painter = ui.painter();
    painter.rect_filled(preview, 10.0, p.bg);
    let card = Rect::from_min_max(preview.min + vec2(10.0, 10.0), preview.max - vec2(10.0, 10.0));
    painter.rect(card, 6.0, p.card, Stroke::new(1.0_f32, p.border));
    painter.rect_filled(Rect::from_min_size(card.min + vec2(8.0, 9.0), vec2(40.0, 6.0)), 3.0, p.text);
    painter.rect_filled(Rect::from_min_size(card.min + vec2(8.0, 21.0), vec2(card.width() - 16.0, 10.0)), 3.0, p.accent);
    let outline = if selected {
        Stroke::new(2.0_f32, theme::accent())
    } else if response.hovered() {
        Stroke::new(1.0_f32, theme::border_strong())
    } else {
        Stroke::new(1.0_f32, theme::border())
    };
    painter.rect_stroke(preview, Rounding::same(10.0), outline);
    let color = if selected { theme::text() } else { theme::muted_color() };
    painter.text(pos2(rect.center().x, preview.bottom() + 16.0), Align2::CENTER_CENTER, option.name(), FontId::new(13.5, theme::semibold()), color);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
