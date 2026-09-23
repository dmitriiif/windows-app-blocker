//! A 24-hour, video-editor style timeline for editing blocking periods.
//!
//! Each lane is one day (or group of days). Blocks can be dragged to move them, their edges
//! dragged to resize them, and double-clicking empty space adds one. A block that runs past
//! midnight shows an arrow at the right edge, and its carry-over appears hatched at the start of
//! the next day's lane, where its end can also be dragged. Below the lanes, the selected lane's
//! blocks can be typed in exactly.

use super::theme;
use crate::config::{Days, ScheduleMode, Track};
use crate::time::{fits, format_duration, format_hhmm, parse_hhmm, Block, DAY};
use chrono::{Datelike, NaiveDateTime, Timelike};
use egui::{pos2, vec2, Align, Align2, Color32, CursorIcon, FontId, Id, Key, Layout, Pos2, Rect, RichText, Rounding, Sense, Shape, Stroke, Ui};

const HEADER_WIDTH: f32 = 128.0;
const RULER_HEIGHT: f32 = 24.0;
const EDGE_GRAB: f32 = 7.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DragKind {
    Move,
    Start,
    End,
    /// The right edge of an overnight block's carry-over, shown in the next day's lane.
    Tail,
}

#[derive(Clone, Copy, Debug)]
struct Hit {
    lane: Track,
    index: usize,
    kind: DragKind,
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    hit: Hit,
    press_minute: f32,
    original: Block,
}

struct FieldEdit {
    lane: Track,
    index: usize,
    end: bool,
    text: String,
}

pub struct TimelineState {
    pub lane: Track,
    pub block: Option<usize>,
    drag: Option<Drag>,
    field: Option<FieldEdit>,
    pub error: Option<String>,
}

impl Default for TimelineState {
    fn default() -> TimelineState {
        TimelineState { lane: Track::EveryDay, block: None, drag: None, field: None, error: None }
    }
}

impl TimelineState {
    fn select(&mut self, lane: Track, block: Option<usize>) {
        if self.lane != lane || self.block != block {
            self.field = None;
        }
        self.lane = lane;
        self.block = block;
    }
}

/// Everything the timeline edits.
pub struct Schedule<'a> {
    pub mode: &'a mut ScheduleMode,
    pub days: &'a mut Days,
}

fn snap(minute: f32, step: i32) -> i32 {
    ((minute / step as f32).round() as i32) * step
}

/// Walks from `target` back towards `original` until `make(value)` fits among the other blocks.
fn clamp_to_free(blocks: &[Block], index: usize, original: i32, target: i32, make: impl Fn(i32) -> Block) -> Block {
    let direction = if target > original { -1 } else { 1 };
    let mut value = target;
    loop {
        let candidate = make(value);
        if fits(blocks, Some(index), &candidate) || value == original {
            return candidate;
        }
        value += direction;
    }
}

/// Sorts a lane by start time and returns the new index of the block that was at `index`.
fn sort_lane(blocks: &mut [Block], index: Option<usize>) -> Option<usize> {
    let keep = index.and_then(|i| blocks.get(i).copied());
    blocks.sort_by_key(|b| b.start);
    keep.and_then(|b| blocks.iter().position(|x| *x == b))
}

/// Finds room for a new block near `preferred` (in minutes), trying shorter lengths if needed.
fn place_new_block(blocks: &[Block], preferred: i32) -> Option<Block> {
    for len in [60, 45, 30, 15, 5] {
        for offset in (0..DAY as i32).step_by(15) {
            let start = (preferred + offset).rem_euclid(DAY as i32);
            let candidate = Block::from_start_len(start, len);
            if fits(blocks, None, &candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// What is under the pointer: block edges first (so narrow blocks can still be resized), then
/// bodies, with a lane's own blocks taking priority over carry-overs.
fn hit_test(hits: &[(Rect, Hit)], pos: Pos2) -> Option<Hit> {
    let mut best: Option<(f32, Hit)> = None;
    for (rect, hit) in hits {
        if pos.y < rect.top() - 4.0 || pos.y > rect.bottom() + 4.0 {
            continue;
        }
        let edges: &[(f32, DragKind)] = match hit.kind {
            DragKind::Tail => &[(rect.right(), DragKind::Tail)],
            _ => &[(rect.left(), DragKind::Start), (rect.right(), DragKind::End)],
        };
        let grab = EDGE_GRAB.min(rect.width() / 3.0).max(3.0);
        for (x, kind) in edges {
            let distance = (pos.x - x).abs();
            if distance <= grab && best.is_none_or(|(d, _)| distance < d) {
                best = Some((distance, Hit { kind: *kind, ..*hit }));
            }
        }
    }
    if let Some((_, hit)) = best {
        return Some(hit);
    }
    hits.iter().rev().find(|(rect, _)| rect.contains(pos)).map(|(_, hit)| match hit.kind {
        DragKind::Tail => Hit { kind: DragKind::Move, ..*hit },
        _ => *hit,
    })
}

fn minute_of(at: NaiveDateTime) -> f32 {
    (at.hour() * 60 + at.minute()) as f32 + at.second() as f32 / 60.0
}

/// Draws the schedule editor. Returns true if anything changed.
pub fn show(ui: &mut Ui, state: &mut TimelineState, schedule: Schedule, editable: bool, now: NaiveDateTime) -> bool {
    let Schedule { mode, days } = schedule;
    let mut changed = false;

    let lanes = Track::lanes(*mode);
    if !lanes.contains(&state.lane) {
        let today = Track::for_day(*mode, now.weekday().num_days_from_monday());
        state.select(today, None);
    }

    ui.horizontal(|ui| {
        ui.add_enabled_ui(editable, |ui| {
            let options = [
                (ScheduleMode::EveryDay, "Same every day"),
                (ScheduleMode::WeekdayWeekend, "Weekdays / weekend"),
                (ScheduleMode::IndividualDays, "Each day"),
            ];
            if theme::segmented(ui, mode, &options) {
                changed = true;
                let today = Track::for_day(*mode, now.weekday().num_days_from_monday());
                state.select(today, None);
                state.drag = None;
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(theme::muted("Drag to move · drag edges to resize · double-click to add").size(12.5));
        });
    });
    ui.add_space(6.0);

    changed |= lanes_view(ui, state, days, *mode, editable, now);
    ui.add_space(10.0);
    changed |= block_list(ui, state, days, *mode, editable);
    changed
}

fn lanes_view(ui: &mut Ui, state: &mut TimelineState, days: &mut Days, mode: ScheduleMode, editable: bool, now: NaiveDateTime) -> bool {
    let lanes = Track::lanes(mode);
    let lane_height = if lanes.len() > 2 { 36.0 } else { 46.0 };
    let gap = 6.0;
    let width = ui.available_width().max(420.0);
    let height = RULER_HEIGHT + lanes.len() as f32 * (lane_height + gap);
    let (outer, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let track_left = outer.left() + HEADER_WIDTH;
    let track_width = outer.right() - track_left - 4.0;
    let x_of = |minute: f32| track_left + minute / DAY as f32 * track_width;
    let minute_at = |x: f32| (x - track_left) / track_width * DAY as f32;
    let painter = ui.painter_at(outer);
    let today = Track::for_day(mode, now.weekday().num_days_from_monday());
    let shift = ui.input(|i| i.modifiers.shift);
    let step = if shift { 1 } else { 5 };
    let mut changed = false;

    // Hour ruler.
    let px_per_hour = track_width / 24.0;
    let label_every = if px_per_hour >= 34.0 { 1 } else if px_per_hour >= 18.0 { 2 } else { 3 };
    let lanes_top = outer.top() + RULER_HEIGHT;
    let lanes_bottom = outer.bottom() - gap;
    for hour in 0..=24 {
        let x = x_of(hour as f32 * 60.0);
        let major = hour % 6 == 0;
        painter.line_segment(
            [pos2(x, lanes_top - 4.0), pos2(x, lanes_bottom)],
            Stroke::new(1.0_f32, if major { theme::grid_major() } else { theme::grid() }),
        );
        // Labels under the "now" marker are hidden so the two do not overlap.
        if hour % label_every == 0 && (x - x_of(minute_of(now))).abs() > 30.0 {
            let align = match hour {
                0 => Align2::LEFT_CENTER,
                24 => Align2::RIGHT_CENTER,
                _ => Align2::CENTER_CENTER,
            };
            painter.text(pos2(x, outer.top() + 9.0), align, format!("{hour:02}"), FontId::proportional(11.5), if major { theme::muted_color() } else { theme::faint() });
        }
    }

    for (row, &lane) in lanes.iter().enumerate() {
        let top = lanes_top + row as f32 * (lane_height + gap);
        let header = Rect::from_min_size(pos2(outer.left(), top), vec2(HEADER_WIDTH - 10.0, lane_height));
        let track = Rect::from_min_max(pos2(track_left, top), pos2(track_left + track_width, top + lane_height));
        let enabled = days.is_enabled(lane);
        let selected_lane = state.lane == lane;

        // Lane header: name, "today" marker and on/off switch.
        let mut header_ui = ui.new_child(egui::UiBuilder::new().max_rect(header).layout(Layout::left_to_right(Align::Center)));
        let name_color = if enabled { theme::text() } else { theme::faint() };
        let name = RichText::new(if lanes.len() > 2 { lane.short_name() } else { lane.name() })
            .font(FontId::new(14.0, if selected_lane { theme::semibold() } else { egui::FontFamily::Proportional }))
            .color(name_color);
        if header_ui.add(egui::Label::new(name).sense(Sense::click())).clicked() {
            state.select(lane, None);
        }
        if lane == today {
            header_ui.add_space(-4.0);
            let (dot, _) = header_ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
            header_ui.painter().circle_filled(dot.center(), 3.5, theme::red());
        }
        if lane != Track::EveryDay {
            header_ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_enabled_ui(editable, |ui| {
                    let schedule = days.get_mut(lane);
                    let response = theme::toggle(ui, &mut schedule.enabled).on_hover_text(if schedule.enabled {
                        "Blocks start on this day. Switch off to start none."
                    } else {
                        "No blocks start on this day. Overnight blocks from the day before still finish."
                    });
                    changed |= response.changed();
                });
            });
        }

        // Lane background.
        let fill = if lane == today { theme::today() } else { theme::card_color() };
        painter.rect_filled(track, Rounding::same(8.0), fill);
        painter.rect_stroke(track, Rounding::same(8.0), Stroke::new(1.0_f32, if selected_lane { theme::accent().linear_multiply(0.7) } else { theme::border() }));
        let lane_painter = painter.with_clip_rect(track.shrink(1.0));
        let inner = track.shrink2(vec2(0.0, 5.0));

        // Carry-over from the previous day, hatched.
        let mut hits: Vec<(Rect, Hit)> = Vec::new();
        for &source in lane.carry_sources() {
            if !days.is_enabled(source) {
                continue;
            }
            for (index, block) in days.get(source).blocks.iter().enumerate() {
                if !block.is_overnight() {
                    continue;
                }
                let rect = Rect::from_min_max(pos2(track.left(), inner.top()), pos2(x_of(block.end as f32), inner.bottom()));
                let selected = state.lane == source && state.block == Some(index);
                // A carry-over applies even when this lane is off, so it keeps its colour.
                let color = theme::accent();
                lane_painter.rect_filled(rect, Rounding { nw: 0.0, sw: 0.0, ne: 6.0, se: 6.0 }, color.linear_multiply(0.14));
                let hatch = lane_painter.with_clip_rect(rect.intersect(track.shrink(1.0)));
                let mut x = rect.left() - rect.height();
                while x < rect.right() {
                    hatch.line_segment([pos2(x, rect.bottom()), pos2(x + rect.height(), rect.top())], Stroke::new(1.5_f32, color.linear_multiply(0.35)));
                    x += 7.0;
                }
                lane_painter.line_segment(
                    [pos2(rect.right() - 1.0, rect.top()), pos2(rect.right() - 1.0, rect.bottom())],
                    Stroke::new(if selected { 3.0_f32 } else { 2.0_f32 }, color.linear_multiply(if selected { 1.0 } else { 0.7 })),
                );
                hits.push((rect, Hit { lane: source, index, kind: DragKind::Tail }));
            }
        }

        // This lane's own blocks.
        let blocks = &days.get(lane).blocks;
        for (index, block) in blocks.iter().enumerate() {
            let selected = state.lane == lane && state.block == Some(index);
            let end = block.end_abs().min(DAY) as f32;
            let rect = Rect::from_min_max(pos2(x_of(block.start as f32), inner.top()), pos2(x_of(end), inner.bottom()));
            let base = if enabled { theme::accent() } else { theme::block_off() };
            let fill = if selected { theme::accent_hover() } else { base.linear_multiply(0.85) };
            let ink = if enabled { theme::on_accent() } else { Color32::WHITE };
            let rounding = if block.is_overnight() { Rounding { nw: 6.0, sw: 6.0, ne: 0.0, se: 0.0 } } else { Rounding::same(6.0) };
            lane_painter.rect_filled(rect, rounding, fill);
            if selected {
                lane_painter.rect_stroke(rect.shrink(0.5), rounding, Stroke::new(1.5_f32, ink));
            }
            // Grips on the edges.
            for x in [rect.left() + 4.0, rect.right() - 4.0] {
                if rect.width() > 26.0 && !(block.is_overnight() && x > rect.center().x) {
                    lane_painter.line_segment([pos2(x, rect.center().y - 6.0), pos2(x, rect.center().y + 6.0)], Stroke::new(1.5_f32, ink.gamma_multiply(0.5)));
                }
            }
            if block.is_overnight() {
                let tip = pos2(rect.right() - 3.0, rect.center().y);
                lane_painter.add(Shape::convex_polygon(
                    vec![pos2(tip.x - 9.0, tip.y - 7.0), tip, pos2(tip.x - 9.0, tip.y + 7.0)],
                    ink.gamma_multiply(0.85),
                    Stroke::NONE,
                ));
            }
            let label = block.label();
            let font = FontId::new(12.5, theme::semibold());
            let label_width = ui.fonts(|f| f.layout_no_wrap(label.clone(), font.clone(), Color32::WHITE).size().x);
            if rect.width() > label_width + 22.0 {
                lane_painter.text(rect.center(), Align2::CENTER_CENTER, label, font, ink);
            }
            hits.push((rect, Hit { lane, index, kind: DragKind::Move }));
        }

        // Interaction.
        let response = ui.interact(track, Id::new(("timeline-lane", lane)), Sense::click_and_drag());
        let hit_at = |pos: Pos2| hit_test(&hits, pos);

        if let Some(pos) = response.hover_pos() {
            if state.drag.is_none() {
                match hit_at(pos) {
                    Some(Hit { kind: DragKind::Move, lane: owner, .. }) if editable && owner == lane => ui.ctx().set_cursor_icon(CursorIcon::Grab),
                    Some(Hit { kind: DragKind::Start | DragKind::End | DragKind::Tail, .. }) if editable => {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal)
                    }
                    Some(_) => ui.ctx().set_cursor_icon(CursorIcon::PointingHand),
                    None => {}
                }
            }
        }

        if response.clicked() {
            let hit = response.interact_pointer_pos().and_then(hit_at);
            match hit {
                Some(hit) => state.select(hit.lane, Some(hit.index)),
                None => state.select(lane, None),
            }
        }

        if response.double_clicked() && editable {
            if let Some(pos) = response.interact_pointer_pos() {
                if hit_at(pos).is_none() {
                    let start = snap(minute_at(pos.x), 5).clamp(0, DAY as i32 - 5);
                    let blocks = &mut days.get_mut(lane).blocks;
                    let mut added = None;
                    for len in [60, 45, 30, 15, 5] {
                        let candidate = Block::from_start_len(start, len);
                        if fits(blocks, None, &candidate) {
                            added = Some(candidate);
                            break;
                        }
                    }
                    if let Some(block) = added {
                        blocks.push(block);
                        let last = blocks.len() - 1;
                        let index = sort_lane(blocks, Some(last));
                        state.select(lane, index);
                        changed = true;
                    }
                }
            }
        }

        if response.drag_started() && editable {
            if let Some(origin) = ui.input(|i| i.pointer.press_origin()) {
                if let Some(hit) = hit_at(origin) {
                    // Carry-over tails can only be resized from their end; moving happens in the owning lane.
                    if !(hit.kind == DragKind::Move && hit.lane != lane) {
                        let original = days.get(hit.lane).blocks[hit.index];
                        state.drag = Some(Drag { hit, press_minute: minute_at(origin.x), original });
                    }
                    state.select(hit.lane, Some(hit.index));
                }
            }
        }

        let dragging_here = response.dragged() && state.drag.is_some();
        if dragging_here {
            let drag = state.drag.expect("checked above");
            if let Some(pointer) = response.interact_pointer_pos() {
                let pointer_minute = minute_at(pointer.x);
                let o = drag.original;
                let blocks = &mut days.get_mut(drag.hit.lane).blocks;
                let updated = match drag.hit.kind {
                    DragKind::Move => {
                        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                        let target = snap(o.start as f32 + pointer_minute - drag.press_minute, step).clamp(0, DAY as i32 - 1);
                        clamp_to_free(blocks, drag.hit.index, o.start as i32, target, |s| Block::from_start_len(s, o.minutes() as i32))
                    }
                    DragKind::Start => {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                        let end = o.end_abs() as i32;
                        let low = (end - (DAY as i32 - 1)).max(0);
                        let high = (end - step).min(DAY as i32 - 1);
                        let target = snap(pointer_minute, step).clamp(low, high.max(low));
                        clamp_to_free(blocks, drag.hit.index, o.start as i32, target, |s| Block::from_start_len(s, end - s))
                    }
                    DragKind::End => {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                        let low = o.start as i32 + step;
                        let target = snap(pointer_minute, step).clamp(low.min(DAY as i32), DAY as i32);
                        let original_end = (o.end_abs() as i32).min(DAY as i32);
                        clamp_to_free(blocks, drag.hit.index, original_end, target, |e| Block::from_start_len(o.start as i32, e - o.start as i32))
                    }
                    DragKind::Tail => {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                        let high = (o.start as i32 - step).max(0);
                        let target = snap(pointer_minute, step).clamp(0, high);
                        let original_end = if o.is_overnight() { o.end as i32 } else { 0 };
                        clamp_to_free(blocks, drag.hit.index, original_end, target, |e| {
                            Block::from_start_len(o.start as i32, DAY as i32 + e - o.start as i32)
                        })
                    }
                };
                if blocks[drag.hit.index] != updated {
                    blocks[drag.hit.index] = updated;
                    changed = true;
                }
                // Floating time readout.
                let text = format!("{}  ·  {}", updated.label(), format_duration(updated.minutes() as u32));
                let tip = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("timeline-drag-tip")));
                let galley = ui.fonts(|f| f.layout_no_wrap(text, FontId::new(13.0, theme::semibold()), Color32::WHITE));
                let at = pos2(pointer.x - galley.size().x / 2.0, track.top() - galley.size().y - 14.0);
                let bubble = Rect::from_min_size(at, galley.size()).expand2(vec2(8.0, 4.0));
                tip.rect_filled(bubble, 6.0, theme::inset());
                tip.rect_stroke(bubble, 6.0, Stroke::new(1.0_f32, theme::accent()));
                tip.galley(at, galley, theme::text());
            }
        }

        if response.drag_stopped() {
            if let Some(drag) = state.drag.take() {
                let blocks = &mut days.get_mut(drag.hit.lane).blocks;
                let index = sort_lane(blocks, Some(drag.hit.index));
                state.select(drag.hit.lane, index);
            }
        }
    }

    // "Now" playhead across every lane.
    let x = x_of(minute_of(now));
    painter.line_segment([pos2(x, lanes_top - 2.0), pos2(x, lanes_bottom)], Stroke::new(2.0_f32, theme::red()));
    let label = format_hhmm((now.hour() * 60 + now.minute()) as u16);
    let galley = ui.fonts(|f| f.layout_no_wrap(label, FontId::new(11.0, theme::semibold()), Color32::WHITE));
    let pill = Rect::from_center_size(pos2(x, outer.top() + 9.0), galley.size() + vec2(10.0, 4.0));
    let pill = pill.translate(vec2(
        (track_left - pill.left()).max(0.0) + (outer.right() - pill.right()).min(0.0),
        0.0,
    ));
    painter.rect_filled(pill, 5.0, theme::red());
    painter.galley(pill.center() - galley.size() / 2.0, galley, Color32::WHITE);

    // Delete key removes the selected block.
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    if editable && !typing && ui.input(|i| i.key_pressed(Key::Delete)) {
        if let Some(index) = state.block {
            let blocks = &mut days.get_mut(state.lane).blocks;
            if index < blocks.len() {
                blocks.remove(index);
                state.select(state.lane, None);
                changed = true;
            }
        }
    }
    changed
}

fn block_list(ui: &mut Ui, state: &mut TimelineState, days: &mut Days, mode: ScheduleMode, editable: bool) -> bool {
    let mut changed = false;
    let lane = state.lane;
    let count = days.get(lane).blocks.len();

    ui.horizontal(|ui| {
        ui.label(theme::title(lane.name(), 15.0));
        let summary = match count {
            0 => "no blocks".to_owned(),
            1 => "1 block".to_owned(),
            n => format!("{n} blocks"),
        };
        ui.label(theme::muted(summary));
        if lane != Track::EveryDay && !days.get(lane).enabled {
            theme::badge(ui, "OFF", theme::amber());
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_enabled_ui(editable, |ui| {
                let targets: Vec<Track> = Track::lanes(mode).iter().copied().filter(|t| *t != lane).collect();
                if !targets.is_empty() {
                    ui.menu_button("Copy to…", |ui| {
                        let blocks = days.get(lane).blocks.clone();
                        if targets.len() > 1 && ui.button("All other days").clicked() {
                            for t in &targets {
                                days.get_mut(*t).blocks = blocks.clone();
                            }
                            changed = true;
                            ui.close_menu();
                        }
                        for t in &targets {
                            if ui.button(t.name()).clicked() {
                                days.get_mut(*t).blocks = blocks.clone();
                                changed = true;
                                ui.close_menu();
                            }
                        }
                    });
                }
                let blocks = &mut days.get_mut(lane).blocks;
                let add = ui.add_enabled(place_new_block(blocks, 9 * 60).is_some(), egui::Button::new(RichText::new("+  Add block").color(theme::text())).fill(theme::raised()));
                if add.clicked() {
                    if let Some(block) = place_new_block(blocks, 9 * 60) {
                        blocks.push(block);
                        let last = blocks.len() - 1;
                        let index = sort_lane(blocks, Some(last));
                        state.select(lane, index);
                        changed = true;
                    }
                }
            });
        });
    });

    if count == 0 {
        ui.label(theme::muted("Nothing is blocked on this lane. Double-click the timeline or use “Add block”."));
    }

    let mut remove: Option<usize> = None;
    for index in 0..count {
        let block = days.get(lane).blocks[index];
        let selected = state.block == Some(index);
        let frame = egui::Frame::none()
            .fill(if selected { theme::row_selected() } else { theme::row() })
            .stroke(Stroke::new(1.0_f32, if selected { theme::accent().linear_multiply(0.6) } else { theme::border() }))
            .rounding(Rounding::same(10.0))
            .inner_margin(egui::Margin::symmetric(12.0, 6.0));
        let row = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (swatch, _) = ui.allocate_exact_size(vec2(6.0, 22.0), Sense::hover());
                ui.painter().rect_filled(swatch, 3.0, theme::accent());
                ui.add_enabled_ui(editable, |ui| {
                    ui.label(theme::muted("From"));
                    changed |= time_field(ui, state, days, lane, index, false);
                    ui.label(theme::muted("until"));
                    changed |= time_field(ui, state, days, lane, index, true);
                });
                ui.label(theme::muted(format_duration(block.minutes() as u32)));
                if block.is_overnight() {
                    theme::badge(ui, "OVERNIGHT", theme::amber()).on_hover_text("Runs past midnight and finishes the next morning.");
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let delete = egui::Button::new(RichText::new("×").size(18.0).color(theme::muted_color())).fill(Color32::TRANSPARENT).stroke(Stroke::NONE);
                    if ui.add_enabled(editable, delete).on_hover_text("Remove this block").clicked() {
                        remove = Some(index);
                    }
                });
            });
        });
        // Checked passively so the row does not steal clicks from its own fields and buttons.
        let clicked = ui.rect_contains_pointer(row.response.rect) && ui.input(|i| i.pointer.primary_clicked());
        if clicked && remove.is_none() {
            state.select(lane, Some(index));
        }
    }
    if let Some(index) = remove {
        days.get_mut(lane).blocks.remove(index);
        state.select(lane, None);
        changed = true;
    }

    if let Some(error) = &state.error {
        ui.label(RichText::new(error).color(theme::red()));
    }
    ui.label(theme::muted("The start time is included; the end time is not. End at 24:00 to block until midnight, or before the start time to run overnight.").size(12.5));
    changed
}

/// An `HH:MM` text field. Applies on Enter or when focus leaves it.
fn time_field(ui: &mut Ui, state: &mut TimelineState, days: &mut Days, lane: Track, index: usize, end: bool) -> bool {
    let block = days.get(lane).blocks[index];
    let current = format_hhmm(if end { block.end } else { block.start });
    let editing = state.field.as_ref().is_some_and(|f| f.lane == lane && f.index == index && f.end == end);
    let mut text = if editing { state.field.as_ref().map(|f| f.text.clone()).unwrap_or_default() } else { current.clone() };
    let response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .desired_width(54.0)
            .font(FontId::new(14.5, theme::semibold()))
            .char_limit(5)
            .id(Id::new(("time-field", lane, index, end))),
    );
    if response.gained_focus() || response.has_focus() {
        state.field = Some(FieldEdit { lane, index, end, text: text.clone() });
        state.select_keep_field(lane, index);
    }
    if !response.lost_focus() {
        return false;
    }
    state.field = None;
    if text.trim() == current {
        return false;
    }
    let parsed = parse_hhmm(&text, end).and_then(|minute| {
        let (start, finish) = if end { (block.start, minute) } else { (minute, block.end) };
        Block::new(start, finish)
    });
    let blocks = &mut days.get_mut(lane).blocks;
    match parsed {
        Ok(candidate) if fits(blocks, Some(index), &candidate) => {
            blocks[index] = candidate;
            let new_index = sort_lane(blocks, Some(index));
            state.select(lane, new_index);
            state.error = None;
            true
        }
        Ok(_) => {
            state.error = Some("That time would overlap another block on this lane.".into());
            false
        }
        Err(message) => {
            state.error = Some(message);
            false
        }
    }
}

impl TimelineState {
    fn select_keep_field(&mut self, lane: Track, block: usize) {
        self.lane = lane;
        self.block = Some(block);
    }
}
