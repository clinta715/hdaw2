pub mod automation;
mod clips;
mod auto_interaction;
mod interaction;
mod playhead;
mod ruler;
mod track_headers;

use crate::app::HdawApp;
use egui::{pos2, vec2, Color32, Rect, Response, Sense, Ui};
use std::sync::atomic::Ordering;

const RULER_HEIGHT: f32 = 20.0;
pub const CLIP_CORNER_RADIUS: f32 = 3.0;
pub const PLAYHEAD_WIDTH: f32 = 2.0;
pub const DEFAULT_HEADER_WIDTH: f32 = 220.0;
pub const DEFAULT_TRACK_HEIGHT: f32 = 80.0;

pub struct TimelineState {
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub pixels_per_second: f64,
    pub selected_clip_id: Option<uuid::Uuid>,
    pub drag_state: Option<DragState>,
    pub auto_drag: Option<AutoDragState>,
    pub snap_enabled: bool,
    pub loop_drag: Option<LoopDragState>,
    pub header_width: f32,
    pub track_height: f32,
    pub track_context_menu: Option<usize>,
}

pub struct LoopDragState {
    pub handle: LoopHandle,
    pub drag_start_x: f64,
    pub original_frame: u64,
}

#[derive(Clone, Copy)]
pub enum LoopHandle {
    In,
    Out,
}

impl TimelineState {
    pub fn grid_step_secs(&self) -> f64 {
        let pps = self.pixels_per_second;
        if pps > 200.0 { 0.5 }
        else if pps > 80.0 { 1.0 }
        else if pps > 30.0 { 5.0 }
        else { 10.0 }
    }

    pub fn snap_frames_to_grid(&self, frames: u64, sr: u32) -> u64 {
        if !self.snap_enabled { return frames; }
        let step = self.grid_step_secs();
        let t = frames as f64 / sr as f64;
        let snapped = (t / step).round() * step;
        (snapped * sr as f64).round().max(0.0) as u64
    }
}

pub struct DragState {
    pub clip_id: uuid::Uuid,
    pub track_index: usize,
    pub drag_start_x: f64,
    pub original_position_frames: u64,
    pub original_offset_frames: u64,
    pub original_length_frames: u64,
    pub mode: DragMode,
}

pub struct AutoDragState {
    pub lane_index: usize,
    pub point_index: usize,
    pub old_value: f32,
}

pub enum DragMode {
    Move,
    TrimLeft,
    TrimRight,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            scroll_x: 0.0,
            scroll_y: 0.0,
            pixels_per_second: 60.0,
            selected_clip_id: None,
            drag_state: None,
            auto_drag: None,
            snap_enabled: true,
            loop_drag: None,
            header_width: DEFAULT_HEADER_WIDTH,
            track_height: DEFAULT_TRACK_HEIGHT,
            track_context_menu: None,
        }
    }
}

pub fn render(ui: &mut Ui, app: &mut HdawApp) {
    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());

    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter_at(rect);

    let header_width = app.timeline_state.header_width;
    let track_height = app.timeline_state.track_height;

    handle_zoom_and_scroll(ui, &response, &rect, app, header_width);
    clamp_scroll_y(&rect, app, track_height);

    let bg = Color32::from_rgb(0x1a, 0x1a, 0x1a);
    painter.rect_filled(rect, 0.0, bg);

    let sr = app.engine.transport.sample_rate();

    let (loop_in, loop_out) = app.engine.transport.load_loop_region();
    ruler::draw(
        &painter,
        rect,
        &app.timeline_state,
        &app.project.markers,
        Some(loop_in),
        Some(loop_out),
        app.engine.transport.loop_enabled.load(Ordering::Acquire),
        sr,
    );
    draw_grid_lines(&painter, &rect, &app.timeline_state, header_width);

    render_tracks(&painter, &rect, sr, app, header_width, track_height);

    playhead::draw(&painter, rect, &app.timeline_state, app, header_width);
    handle_playhead_follow(&rect, app, header_width);

    interaction::handle_drag_end_snap(&response, app);
    interaction::handle_seek_click(&response, ui, &rect, sr, app, header_width);
    interaction::handle_loop_interaction(&response, ui, &rect, sr, app, header_width);
    app.handle_pool_drop(&response, ui, &rect);
    interaction::handle_clip_interaction(&response, ui, &rect, app, header_width, track_height);
    interaction::handle_track_header_interaction(&response, ui, &rect, app, header_width, track_height);
    auto_interaction::sync_automation_to_project(app);
    auto_interaction::handle_automation_interaction(&response, &rect, app, header_width, track_height);
    handle_track_context_menu(ui, app);
}

fn handle_track_context_menu(ui: &Ui, app: &mut HdawApp) {
    let track_idx = match app.timeline_state.track_context_menu {
        Some(i) if i < app.track_ui.len() => i,
        _ => return,
    };

    let name = &app.track_ui[track_idx].name;
    let mut close = false;

    egui::Window::new(format!("Track: {name}"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
        .show(ui.ctx(), |ui| {
            if ui.button("Add Effect...").clicked() {
                app.effect_editor_state.selected_track = Some(track_idx);
                app.effect_editor_state.show_add_menu = true;
                app.effect_editor_state.show_editor = true;
                close = true;
            }

            ui.separator();
            if ui.button("New MIDI Clip").clicked() {
                let sr = app.engine.transport.sample_rate();
                let pos = app.engine.transport.position_frames();
                let len = (sr as u64).max(4); // 1 second default
                app.add_midi_clip(track_idx, pos, len);
                close = true;
            }

            let instruments: Vec<_> = app.plugin_registry.iter()
                .filter(|d| d.is_instrument)
                .cloned().collect();
            if !instruments.is_empty() {
                ui.separator();
                ui.label("Add Instrument Track");
                for desc in &instruments {
                    if ui.button(&desc.name).clicked() {
                        app.add_instrument_track(desc);
                        close = true;
                    }
                }
            }

            ui.separator();
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });

    if close {
        app.timeline_state.track_context_menu = None;
    }
}

fn handle_zoom_and_scroll(ui: &Ui, response: &Response, rect: &Rect, app: &mut HdawApp, header_width: f32) {
    let mw_delta = ui.input(|i| i.raw_scroll_delta);
    if mw_delta.y != 0.0 {
        let factor = 1.0 - mw_delta.y as f64 * 0.002;
        let old_pps = app.timeline_state.pixels_per_second;
        app.timeline_state.pixels_per_second =
            (app.timeline_state.pixels_per_second * factor).clamp(10.0, 500.0);
        let mouse_x = ui.input(|i| i.pointer.hover_pos()).map_or(0.0, |p| p.x);
        let timeline_x = mouse_x - rect.left() - header_width;
        if timeline_x > 0.0 {
            let time_at_mouse =
                (timeline_x as f64 + app.timeline_state.scroll_x) / old_pps;
            app.timeline_state.scroll_x =
                timeline_x as f64 - time_at_mouse * app.timeline_state.pixels_per_second;
        }
    }

    if response.dragged_by(egui::PointerButton::Middle) {
        app.timeline_state.scroll_x -= response.drag_delta().x as f64;
        app.timeline_state.scroll_y -= response.drag_delta().y as f64;
    }
}

fn clamp_scroll_y(rect: &Rect, app: &mut HdawApp, track_height: f32) {
    let max_scroll = (app.track_ui.len() as f64 * track_height as f64
        - rect.height() as f64
        + RULER_HEIGHT as f64)
        .max(0.0);
    app.timeline_state.scroll_y = app
        .timeline_state
        .scroll_y
        .min(0.0)
        .max(-max_scroll);
}

fn draw_grid_lines(painter: &egui::Painter, rect: &Rect, state: &TimelineState, header_width: f32) {
    if !state.snap_enabled { return; }
    let step = state.grid_step_secs();
    let pps = state.pixels_per_second;
    let start_time = (state.scroll_x / pps).max(0.0);
    let end_time = (state.scroll_x + rect.width() as f64) / pps;
    let first_tick = (start_time / step).ceil() * step;
    let mut t = first_tick;
    let grid_color = Color32::from_rgba_premultiplied(60, 60, 60, 30);
    while t <= end_time {
        let x = rect.left() + (t * pps - state.scroll_x) as f32;
        if x >= rect.left() + header_width && x <= rect.right() {
            let top_y = rect.top() + RULER_HEIGHT;
            painter.line_segment(
                [pos2(x, top_y), pos2(x, rect.bottom())],
                egui::Stroke::new(1.0, grid_color),
            );
        }
        t += step;
    }
}

fn render_tracks(painter: &egui::Painter, rect: &Rect, sr: u32, app: &mut HdawApp, header_width: f32, track_height: f32) {
    for (i, track_ui) in app.track_ui.iter().enumerate() {
        let track_y = rect.top() + RULER_HEIGHT + i as f32 * track_height
            + app.timeline_state.scroll_y as f32;
        if track_y + track_height < rect.top() || track_y > rect.bottom() {
            continue;
        }

        let header_rect = Rect::from_min_size(
            pos2(rect.left(), track_y),
            vec2(header_width, track_height),
        );
        let lane_rect = Rect::from_min_size(
            pos2(rect.left() + header_width, track_y),
            vec2((rect.width() - header_width).max(0.0), track_height),
        );

        let is_selected = app.selected_track == Some(i);
        track_headers::draw(painter, &header_rect, track_ui, is_selected);

        let lane_bg = Color32::from_rgb(0x22, 0x22, 0x22);
        painter.rect_filled(lane_rect, 0.0, lane_bg);

        if let Some(track) = app.project.tracks.get(i) {
            for clip in &track.clips {
                clips::draw(painter, &lane_rect, clip, &app.timeline_state, sr);
            }
        }

        if app.selected_track == Some(i) {
            if let Ok(tracks) = app.engine.tracks.lock() {
                if let Some(handle) = tracks.get(i) {
                    automation::draw(painter, &lane_rect, &handle.automation_lanes, &app.timeline_state, sr);
                }
            }
        }
    }
}

fn handle_playhead_follow(rect: &Rect, app: &mut HdawApp, header_width: f32) {
    if !app.is_playing() { return; }
    let pps = app.timeline_state.pixels_per_second;
    let lane_width = (rect.width() - header_width) as f64;
    let pos_secs = app.position_seconds();
    let playhead_local = pos_secs * pps - app.timeline_state.scroll_x;
    let threshold = lane_width * 0.75;
    if playhead_local > threshold {
        app.timeline_state.scroll_x = pos_secs * pps - threshold;
    }
}
