//! The settings dialog: colour theme and the heads-up notification before blocks.

use super::theme;
use super::{dialog_frame, dim_background, App, JobDone};
use crate::config::{self, Theme};
use crate::notify;
use crate::paths;
use egui::{pos2, vec2, Align, Align2, FontId, Id, Layout, LayerId, Order, Rect, Rounding, Sense, Stroke, Ui};

pub(super) fn show(app: &mut App, ctx: &egui::Context) {
    if !app.settings_open {
        return;
    }
    let Some(saved) = app.saved.clone() else { return };
    let mut prefs = saved.preferences;
    let mut close = false;
    let mut test = false;

    dim_background(ctx, "settings-dim");
    let layer = Id::new("settings");
    egui::Area::new(layer).order(Order::Foreground).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
        dialog_frame().show(ui, |ui| {
            ui.set_width(480.0);
            ui.horizontal(|ui| {
                ui.label(theme::title("Settings", 20.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if theme::quiet_button(ui, "Close").clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(6.0);

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
    if test {
        app.start_job("Sending a test notification…", || notify::send_test().map(|_| JobDone::TestNotification));
    }
    if close {
        app.settings_open = false;
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
