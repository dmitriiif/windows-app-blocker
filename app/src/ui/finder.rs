//! The "Find common apps" dialog: suggests installed browsers, game stores and chat apps.

use super::theme;
use super::{dialog_frame, dim_background, App};
use crate::detect::{self, Category, Found};
use crate::paths;
use egui::{vec2, Align, Align2, Id, Layout, LayerId, Order, RichText};

pub(super) struct Finder {
    found: Vec<Found>,
    checked: Vec<bool>,
}

/// Scans the computer and opens the dialog.
pub(super) fn open(app: &mut App) {
    let found = detect::scan(&app.draft.executables);
    let checked = vec![false; found.len()];
    app.finder = Some(Finder { found, checked });
}

pub(super) fn show(app: &mut App, ctx: &egui::Context) {
    let Some(finder) = &mut app.finder else { return };
    let mut close = false;
    let mut add = false;

    dim_background(ctx, "finder-dim");
    let layer = Id::new("finder");
    egui::Area::new(layer).order(Order::Foreground).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
        dialog_frame().show(ui, |ui| {
            ui.set_width(560.0);
            ui.label(theme::title("Common apps on this computer", 20.0));
            ui.label(theme::muted("Browsers, game stores and chat apps found on this PC. Tick the ones to block."));
            ui.add_space(8.0);

            if finder.found.is_empty() {
                ui.label(RichText::new("None of the common apps were found. Use \"Add .exe…\" to choose programs yourself.").color(theme::text()));
            }
            let height = (ctx.screen_rect().height() - 260.0).clamp(160.0, 440.0);
            egui::ScrollArea::vertical().max_height(height).auto_shrink([false, true]).show(ui, |ui| {
                for category in Category::ALL {
                    let rows: Vec<usize> = (0..finder.found.len()).filter(|&i| finder.found[i].category == category).collect();
                    if rows.is_empty() {
                        continue;
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(category.name()).font(egui::FontId::new(14.0, theme::semibold())).color(theme::muted_color()));
                        let open: Vec<usize> = rows.iter().copied().filter(|&i| !finder.found[i].already_added).collect();
                        if !open.is_empty() {
                            let all = open.iter().all(|&i| finder.checked[i]);
                            if ui.small_button(if all { "Clear" } else { "Select all" }).clicked() {
                                open.iter().for_each(|&i| finder.checked[i] = !all);
                            }
                        }
                    });
                    for i in rows {
                        found_row(ui, &finder.found[i], &mut finder.checked[i]);
                    }
                }
            });

            ui.add_space(10.0);
            let count = finder.checked.iter().filter(|c| **c).count();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = match count {
                    0 | 1 => "Add to block list".to_owned(),
                    n => format!("Add {n} apps"),
                };
                if ui.add_enabled_ui(count > 0, |ui| theme::filled_button(ui, &label, vec2(150.0, 36.0))).inner.clicked() {
                    add = true;
                }
                if theme::quiet_button(ui, "Cancel").clicked() {
                    close = true;
                }
            });
        });
    });
    ctx.move_to_top(LayerId::new(Order::Foreground, layer));
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }

    if add {
        let mut added = 0;
        for (found, _) in finder.found.iter().zip(&finder.checked).filter(|(_, c)| **c) {
            if !app.draft.executables.iter().any(|e| paths::same_path(e, &found.path)) {
                app.draft.executables.push(found.path.clone());
                added += 1;
            }
        }
        app.finder = None;
        app.toast(match added {
            1 => "1 app added. Save to start blocking it.".to_owned(),
            n => format!("{n} apps added. Save to start blocking them."),
        });
    } else if close {
        app.finder = None;
    }
}

fn found_row(ui: &mut egui::Ui, found: &Found, checked: &mut bool) {
    ui.horizontal(|ui| {
        ui.add_enabled_ui(!found.already_added, |ui| ui.checkbox(checked, ""));
        theme::app_avatar(ui, found.name);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new(found.name).font(egui::FontId::new(14.5, theme::semibold())));
                if found.already_added {
                    theme::badge(ui, "ADDED", theme::green());
                } else if found.path.contains('*') {
                    theme::badge(ui, "ALL VERSIONS", theme::accent())
                        .on_hover_text("This app installs each update into a new folder. Every version is blocked, so updates do not undo the block.");
                }
            });
            ui.add(egui::Label::new(theme::muted(found.path.as_str()).size(12.0)).truncate());
        });
    });
}
