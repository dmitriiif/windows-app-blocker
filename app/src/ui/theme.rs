//! Colour themes, fonts and small reusable widgets.

use std::cell::Cell;
use crate::config::Theme;
use egui::{
    vec2, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, Response, RichText, Rounding,
    Sense, Stroke, TextStyle, Ui, Vec2, Visuals, WidgetText,
};

/// Every colour the UI uses. One palette per theme; the active one is swapped at runtime.
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub card: Color32,
    pub inset: Color32,
    pub row: Color32,
    pub row_selected: Color32,
    pub today: Color32,
    pub raised: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub grid: Color32,
    pub grid_major: Color32,
    pub toggle_off: Color32,
    pub block_off: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    /// Text and marks drawn on top of an accent fill.
    pub on_accent: Color32,
    pub pink: Color32,
    pub cyan: Color32,
    pub green: Color32,
    pub red: Color32,
    pub amber: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const VIOLET: Palette = Palette {
    dark: true,
    bg: rgb(0x09080d),
    surface: rgb(0x110f18),
    card: rgb(0x161320),
    inset: rgb(0x0e0c14),
    row: rgb(0x13101b),
    row_selected: rgb(0x251b3c),
    today: rgb(0x1d162c),
    raised: rgb(0x221c30),
    hover: rgb(0x2b223d),
    pressed: rgb(0x34294b),
    border: rgb(0x2c243d),
    border_strong: rgb(0x3f3358),
    grid: rgb(0x1c1727),
    grid_major: rgb(0x2b233b),
    toggle_off: rgb(0x3a324e),
    block_off: rgb(0x4a435e),
    text: rgb(0xeeeaf7),
    muted: rgb(0x9d95b3),
    faint: rgb(0x625a76),
    accent: rgb(0x8b5cf6),
    accent_hover: rgb(0xa78bfa),
    on_accent: Color32::WHITE,
    pink: rgb(0xe85dc0),
    cyan: rgb(0x4cc9e0),
    green: rgb(0x34c786),
    red: rgb(0xef5b5b),
    amber: rgb(0xf2a93b),
};

const BLUE: Palette = Palette {
    bg: rgb(0x07090e),
    surface: rgb(0x0d1119),
    card: rgb(0x111621),
    inset: rgb(0x0a0d14),
    row: rgb(0x0f141e),
    row_selected: rgb(0x16253d),
    today: rgb(0x131c2e),
    raised: rgb(0x1a2333),
    hover: rgb(0x212c40),
    pressed: rgb(0x293750),
    border: rgb(0x222c3e),
    border_strong: rgb(0x33425c),
    grid: rgb(0x151b27),
    grid_major: rgb(0x222b3c),
    toggle_off: rgb(0x2e384c),
    block_off: rgb(0x3e485e),
    text: rgb(0xe8eef7),
    muted: rgb(0x929fb5),
    faint: rgb(0x566278),
    accent: rgb(0x3b82f6),
    accent_hover: rgb(0x60a5fa),
    ..VIOLET
};

const GOLD: Palette = Palette {
    bg: rgb(0x0a0907),
    surface: rgb(0x12100c),
    card: rgb(0x171510),
    inset: rgb(0x0d0c09),
    row: rgb(0x14120e),
    row_selected: rgb(0x2b2312),
    today: rgb(0x1f1a10),
    raised: rgb(0x221e16),
    hover: rgb(0x2c271c),
    pressed: rgb(0x362f21),
    border: rgb(0x2c271c),
    border_strong: rgb(0x443b28),
    grid: rgb(0x1a1711),
    grid_major: rgb(0x2a251a),
    toggle_off: rgb(0x3a3428),
    block_off: rgb(0x4c4536),
    text: rgb(0xf3efe6),
    muted: rgb(0xa89f8c),
    faint: rgb(0x665e4e),
    accent: rgb(0xd4a537),
    accent_hover: rgb(0xe6bf5c),
    on_accent: rgb(0x1a1405),
    ..VIOLET
};

const LIGHT: Palette = Palette {
    dark: false,
    bg: rgb(0xf4f3f8),
    surface: rgb(0xffffff),
    card: rgb(0xffffff),
    inset: rgb(0xeeecf4),
    row: rgb(0xf8f7fb),
    row_selected: rgb(0xece5fd),
    today: rgb(0xf1ecfd),
    raised: rgb(0xeceaf2),
    hover: rgb(0xe2dfeb),
    pressed: rgb(0xd6d2e2),
    border: rgb(0xe0dde8),
    border_strong: rgb(0xc9c4d6),
    grid: rgb(0xeeecf3),
    grid_major: rgb(0xddd9e6),
    toggle_off: rgb(0xcfcbda),
    block_off: rgb(0xb8b3c6),
    text: rgb(0x1c1926),
    muted: rgb(0x6b657d),
    faint: rgb(0xa39eb2),
    accent: rgb(0x7c4de8),
    accent_hover: rgb(0x9470ee),
    on_accent: Color32::WHITE,
    pink: rgb(0xc93d9f),
    cyan: rgb(0x1f9bb3),
    green: rgb(0x1f9d62),
    red: rgb(0xd93b3b),
    amber: rgb(0xc47808),
};

pub fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Violet => VIOLET,
        Theme::Blue => BLUE,
        Theme::Gold => GOLD,
        Theme::Light => LIGHT,
    }
}

thread_local! {
    // Already const; clippy misreads the macro expansion.
    #[allow(clippy::missing_const_for_thread_local)]
    static CURRENT: Cell<Palette> = const { Cell::new(VIOLET) };
}

macro_rules! colors {
    ($($name:ident),* $(,)?) => {
        $(pub fn $name() -> Color32 { CURRENT.with(|p| p.get().$name) })*
    };
}

colors!(
    bg, surface, inset, row, row_selected, today, raised, hover, pressed, border, border_strong, grid, grid_major,
    toggle_off, block_off, text, faint, accent, accent_hover, on_accent, pink, cyan, green, red, amber,
);

pub fn is_dark() -> bool {
    CURRENT.with(|p| p.get().dark)
}

pub fn card_color() -> Color32 {
    CURRENT.with(|p| p.get().card)
}

pub fn muted_color() -> Color32 {
    CURRENT.with(|p| p.get().muted)
}

pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let windows_fonts = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| r"C:\Windows".into()).join("Fonts");
    let mut semibold_chain: Vec<String> = Vec::new();
    for (key, file, family) in [("segoe", "segoeui.ttf", FontFamily::Proportional), ("segoe-sb", "seguisb.ttf", semibold())] {
        if let Ok(bytes) = std::fs::read(windows_fonts.join(file)) {
            fonts.font_data.insert(key.to_owned(), FontData::from_owned(bytes));
            if family == FontFamily::Proportional {
                fonts.families.entry(FontFamily::Proportional).or_default().insert(0, key.to_owned());
            } else {
                semibold_chain.push(key.to_owned());
            }
        }
    }
    semibold_chain.extend(fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    fonts.families.insert(semibold(), semibold_chain);
    ctx.set_fonts(fonts);

    ctx.style_mut(|style| {
        style.text_styles.insert(TextStyle::Body, FontId::proportional(14.5));
        style.text_styles.insert(TextStyle::Button, FontId::proportional(14.5));
        style.text_styles.insert(TextStyle::Small, FontId::proportional(12.0));
        style.text_styles.insert(TextStyle::Heading, FontId::new(24.0, semibold()));
        style.text_styles.insert(TextStyle::Monospace, FontId::monospace(14.0));
        style.spacing.item_spacing = vec2(10.0, 10.0);
        style.spacing.button_padding = vec2(14.0, 7.0);
        style.spacing.interact_size = vec2(40.0, 30.0);
    });
}

/// Switches the colour theme for every widget drawn from now on.
pub fn apply(ctx: &egui::Context, theme: Theme) {
    let p = palette(theme);
    CURRENT.with(|c| c.set(p));
    let mut visuals = if p.dark { Visuals::dark() } else { Visuals::light() };
    visuals.panel_fill = bg();
    visuals.window_fill = surface();
    visuals.window_stroke = Stroke::new(1.0_f32, border());
    visuals.window_rounding = Rounding::same(12.0);
    visuals.extreme_bg_color = inset();
    visuals.faint_bg_color = card_color();
    visuals.override_text_color = Some(text());
    visuals.hyperlink_color = accent();
    visuals.selection.bg_fill = accent().linear_multiply(0.55);
    visuals.selection.stroke = Stroke::new(1.0_f32, accent());
    visuals.error_fg_color = red();
    visuals.warn_fg_color = amber();
    for (w, fill, stroke) in [
        (&mut visuals.widgets.noninteractive, card_color(), border()),
        (&mut visuals.widgets.inactive, raised(), border()),
        (&mut visuals.widgets.hovered, hover(), border_strong()),
        (&mut visuals.widgets.active, pressed(), accent()),
        (&mut visuals.widgets.open, raised(), border()),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0_f32, stroke);
        w.rounding = Rounding::same(8.0);
    }
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, text());
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, text());
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border());
    ctx.set_visuals(visuals);
}

pub fn title(text: &str, size: f32) -> RichText {
    RichText::new(text).font(FontId::new(size, semibold())).color(self::text())
}

pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(muted_color())
}

pub fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    Frame::none()
        .fill(card_color())
        .stroke(Stroke::new(1.0_f32, border()))
        .rounding(Rounding::same(14.0))
        .inner_margin(Margin::same(18.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

pub fn section_title(ui: &mut Ui, text: &str, subtitle: Option<&str>) {
    ui.label(title(text, 17.0));
    if let Some(subtitle) = subtitle {
        ui.add_space(-6.0);
        ui.label(muted(subtitle).size(13.0));
    }
    ui.add_space(4.0);
}

/// A filled call-to-action button.
pub fn filled_button(ui: &mut Ui, text: &str, fill: Color32, size: Vec2) -> Response {
    let enabled = ui.is_enabled();
    let fill = if enabled { fill } else { raised() };
    let color = if !enabled {
        faint()
    } else if fill == accent() {
        on_accent()
    } else {
        Color32::WHITE
    };
    ui.add(
        egui::Button::new(RichText::new(text).font(FontId::new(15.0, semibold())).color(color))
            .fill(fill)
            .stroke(Stroke::NONE)
            .rounding(Rounding::same(10.0))
            .min_size(size),
    )
}

pub fn quiet_button(ui: &mut Ui, text: impl Into<WidgetText>) -> Response {
    ui.add(egui::Button::new(text).fill(raised()).rounding(Rounding::same(8.0)))
}

/// An iOS-style on/off switch.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    toggle_colored(ui, on, vec2(36.0, 20.0), accent())
}

/// An on/off switch with a custom size and "on" color.
pub fn toggle_colored(ui: &mut Ui, on: &mut bool, size: Vec2, on_color: Color32) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool(response.id, *on);
        let enabled = ui.is_enabled();
        let off_fill = toggle_off();
        let mut fill = lerp_color(off_fill, on_color, t);
        if !enabled {
            fill = fill.linear_multiply(0.45);
        }
        let radius = rect.height() / 2.0;
        ui.painter().rect_filled(rect, radius, fill);
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
        let knob = if enabled { Color32::WHITE } else { faint() };
        ui.painter().circle_filled(egui::pos2(x, rect.center().y), radius - 3.0, knob);
    }
    response
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// A pill-shaped segmented control. Returns true when the value changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    Frame::none()
        .fill(inset())
        .stroke(Stroke::new(1.0_f32, border()))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::same(3.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (option, label) in options {
                    let selected = *value == *option;
                    let text = RichText::new(*label).size(13.5).color(if selected { on_accent() } else { muted_color() });
                    let button = egui::Button::new(text)
                        .fill(if selected { accent() } else { Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .rounding(Rounding::same(8.0));
                    if ui.add(button).clicked() && !selected {
                        *value = *option;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// A small rounded label, e.g. "Today" or "Locked".
pub fn badge(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::new(11.5, semibold()), color);
    let size = galley.size() + vec2(14.0, 6.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, rect.height() / 2.0, color.linear_multiply(0.16));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}

/// A coloured initial used as an app icon.
pub fn app_avatar(ui: &mut Ui, name: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
    let hash = name.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b.to_ascii_lowercase() as u32));
    let palette = [accent(), green(), amber(), pink(), cyan(), red()];
    let color = palette[(hash as usize) % palette.len()];
    ui.painter().rect_filled(rect, 8.0, color.linear_multiply(0.22));
    let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, initial, FontId::new(15.0, semibold()), color);
}

/// Window icon: a rounded violet tile with a white "no entry" ring.
pub fn window_icon() -> egui::IconData {
    let size = 64u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let c = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let corner = 14.0;
            let (qx, qy) = ((fx.abs() - (c - corner)).max(0.0), (fy.abs() - (c - corner)).max(0.0));
            let inside_tile = (qx * qx + qy * qy).sqrt() <= corner;
            let r = (fx * fx + fy * fy).sqrt();
            let ring = (17.0..=22.0).contains(&r);
            let bar = ((fx + fy) / std::f32::consts::SQRT_2).abs() <= 2.6 && r < 18.0;
            let pixel = if !inside_tile {
                [0, 0, 0, 0]
            } else if ring || bar {
                [255, 255, 255, 255]
            } else {
                let t = (y as f32) / size as f32;
                let mix = |from: f32, to: f32| (from * (1.0 - t) + to * t) as u8;
                [mix(139.0, 91.0), mix(92.0, 33.0), mix(246.0, 182.0), 255]
            };
            rgba.extend_from_slice(&pixel);
        }
    }
    egui::IconData { rgba, width: size, height: size }
}
