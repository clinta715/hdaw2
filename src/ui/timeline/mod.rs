pub mod automation;
mod clips;
mod auto_interaction;
mod interaction;
mod playhead;
mod ruler;
mod track_headers;

use crate::app::HdawApp;
use egui::{pos2, vec2, Color32, Rect, Response, Sense, Ui};

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}
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
    pub ruler_context_menu: Option<RulerContextMenu>,
    pub header_width: f32,
    pub track_height: f32,
    pub track_context_menu: Option<usize>,
    pub clip_context_menu: Option<(usize, uuid::Uuid)>,
}

pub struct RulerContextMenu {
    pub frame: u64,
    pub click_x: f32,
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
    pub fn beat_step(&self, bpm: f64, division: crate::ui::preferences::GridDivision) -> f64 {
        let fixed = division.to_beats();
        if fixed > 0.0 {
            return fixed;
        }

        let pps = self.pixels_per_second;
        let bps = bpm / 60.0;
        let pixels_per_beat = pps / bps;
        
        if pixels_per_beat > 80.0 { 0.25 } // 1/16th notes (assuming 4/4)
        else if pixels_per_beat > 40.0 { 0.5 } // 1/8th notes
        else if pixels_per_beat > 20.0 { 1.0 } // 1/4 notes
        else if pixels_per_beat > 5.0 { 4.0 }  // 1 bar (4/4)
        else { 16.0 } // 4 bars
    }

    pub fn snap_frames_to_grid(&self, frames: u64, sr: u32, bpm: f64, prefs: &crate::ui::preferences::PreferencesState, markers: &[crate::project::marker::Marker]) -> u64 {
        if !self.snap_enabled { return frames; }
        
        let sr_f = sr as f64;
        
        // 1. Try snapping to markers first if enabled
        if prefs.snap_to_markers {
            let threshold_frames = (sr_f * 0.05) as u64; // 50ms snap range
            for marker in markers {
                let dist = (frames as i64 - marker.position_frames as i64).abs();
                if dist < threshold_frames as i64 {
                    return marker.position_frames;
                }
            }
        }

        // 2. Fallback to grid snapping
        let step = self.beat_step(bpm, prefs.grid_division);
        let bps = bpm / 60.0;
        let frames_per_beat = sr_f / bps;
        let frames_step = step * frames_per_beat;
        
        ((frames as f64 / frames_step).round() * frames_step) as u64
    }
}

#[derive(Clone)]
pub struct DragState {
    pub clip_id: uuid::Uuid,
    pub track_index: usize,
    pub original_track_index: usize,
    pub drag_start_x: f64,
    pub original_position_frames: u64,
    pub original_offset_frames: u64,
    pub original_length_frames: u64,
    pub original_fade_in: u64,
    pub original_fade_out: u64,
    pub mode: DragMode,
}

pub struct AutoDragState {
    pub lane_index: usize,
    pub point_index: usize,
    pub old_value: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DragMode {
    Move,
    TrimLeft,
    TrimRight,
    FadeIn,
    FadeOut,
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
            ruler_context_menu: None,
            header_width: DEFAULT_HEADER_WIDTH,
            track_height: DEFAULT_TRACK_HEIGHT,
            track_context_menu: None,
            clip_context_menu: None,
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
    let bpm = app.project.bpm;

    let (loop_in, loop_out) = app.engine.transport.load_loop_region();
    ruler::draw(
        &painter,
        rect,
        header_width,
        &app.timeline_state,
        &app.project.markers,
        Some(loop_in),
        Some(loop_out),
        app.engine.transport.loop_enabled.load(Ordering::Acquire),

        sr,
        bpm,
        &app.project.tempo_events,
        &app.project.time_sig_events,
        &app.preferences,
    );
    draw_grid_lines(&painter, &rect, &app.timeline_state, header_width, bpm, &app.preferences, &app.project.tempo_events, sr);

    // Draw loop region overlay across the full timeline body (below ruler)
    let loop_enabled = app.engine.transport.loop_enabled.load(Ordering::Acquire);
    if loop_enabled && (loop_in > 0 || loop_out > 0) {
        let pps = app.timeline_state.pixels_per_second;
        let sr_f = sr as f64;
        let in_secs = loop_in as f64 / sr_f;
        let out_secs = loop_out as f64 / sr_f;
        let in_x = rect.left() + header_width + (in_secs * pps - app.timeline_state.scroll_x) as f32;
        let out_x = rect.left() + header_width + (out_secs * pps - app.timeline_state.scroll_x) as f32;
        let l = in_x.max(rect.left() + header_width);
        let r = out_x.min(rect.right());
        if r > l {
            let body_loop = Rect::from_min_max(
                pos2(l, rect.top() + RULER_HEIGHT),
                pos2(r, rect.bottom()),
            );
            painter.rect_filled(body_loop, 0.0, Color32::from_rgba_premultiplied(0x44, 0x88, 0xcc, 20));
        }
    }

    // Right-click on ruler area -> show loop set context menu
    if app.timeline_state.ruler_context_menu.is_none()
        && response.clicked_by(egui::PointerButton::Secondary)
    {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            if pos.y <= rect.top() + RULER_HEIGHT && pos.x > rect.left() + header_width {
                let timeline_x = (pos.x - rect.left() - header_width) as f64 + app.timeline_state.scroll_x;
                let time = timeline_x / app.timeline_state.pixels_per_second;
                if time >= 0.0 {
                    let frame = app.timeline_state.snap_frames_to_grid((time * sr as f64) as u64, sr, bpm, &app.preferences, &app.project.markers);
                    app.timeline_state.ruler_context_menu = Some(RulerContextMenu {
                        frame,
                        click_x: pos.x,
                    });
                }
            }
        }
    }

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

    // Double-click below all tracks to create a new track
    if response.double_clicked_by(egui::PointerButton::Primary) {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            if pos.x > rect.left() + header_width {
                let track_y_index = ((pos.y - rect.top() - RULER_HEIGHT - app.timeline_state.scroll_y as f32) / track_height) as i32;
                if track_y_index >= app.track_ui.len() as i32 {
                    app.add_blank_track();
                }
            }
        }
    }

    handle_track_context_menu(ui, app);
    handle_clip_context_menu(ui, app);
    handle_ruler_context_menu(ui, app);
}

fn handle_track_context_menu(ui: &Ui, app: &mut HdawApp) {
    let track_idx = match app.timeline_state.track_context_menu {
        Some(i) if i < app.track_ui.len() => i,
        _ => return,
    };

    let name = &app.track_ui[track_idx].name;
    let mut close = false;

    let has_instrument = if let Ok(tracks) = app.engine.tracks.lock() {
        tracks.get(track_idx).map_or(false, |t| t.fx_chain.iter().any(|e| e.has_note_input))
    } else {
        false
    };

    let instruments: Vec<_> = app.plugin_registry.iter()
        .filter(|d| d.is_instrument)
        .cloned().collect();

    let wr = egui::Window::new(format!("Track: {name}"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
        .show(ui.ctx(), |ui| {
            ui.set_min_width(180.0);

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
                let len = (sr as u64).max(4);
                app.add_midi_clip(track_idx, pos, len);
                close = true;
            }

            if !instruments.is_empty() {
                ui.separator();
                let inst_label = if has_instrument { "Change Instrument" } else { "Assign Instrument" };
                ui.menu_button(inst_label, |ui| {
                    for desc in &instruments {
                        if ui.button(truncate(&desc.name, 30)).clicked() {
                            ui.close_menu();
                            if has_instrument {
                                app.replace_instrument(track_idx, desc);
                            } else {
                                app.assign_instrument(track_idx, desc);
                            }
                            close = true;
                        }
                    }
                });

                ui.menu_button("New Track", |ui| {
                    if ui.button("Blank Track").clicked() {
                        ui.close_menu();
                        app.add_blank_track();
                        close = true;
                    }
                    if ui.button("Group Track").clicked() {
                        ui.close_menu();
                        app.add_group_track();
                        close = true;
                    }
                    if ui.button("Return Track").clicked() {
                        ui.close_menu();
                        app.add_return_track();
                        close = true;
                    }
                    ui.separator();
                    for desc in &instruments {
                        if ui.button(truncate(&format!("New {}", desc.name), 30)).clicked() {
                            ui.close_menu();
                            app.add_instrument_track(desc);
                            close = true;
                        }
                    }
                });
            }

            ui.separator();
            ui.menu_button("Add Automation", |ui| {
                use crate::project::automation::{AutomationLane, PARAM_PAN, PARAM_VOLUME};
                let engine = &app.engine;
                let has_lane = |eid: Option<uuid::Uuid>, pid: u32| -> bool {
                    if let Ok(tracks) = engine.tracks.lock() {
                        tracks.get(track_idx).map_or(false, |t| {
                            t.automation_lanes.iter().any(|l| l.effect_instance_id == eid && l.param_id == pid)
                        })
                    } else { false }
                };

                let mut toggle = |track: usize, eid: Option<uuid::Uuid>, pid: u32, pname: &str| {
                    let lane = match eid {
                        Some(id) => AutomationLane::new_effect(pid, pname.to_string(), id),
                        None => AutomationLane::new(pid, pname.to_string()),
                    };
                    if let Ok(mut ts) = engine.tracks.lock() {
                        if let Some(t) = ts.get_mut(track) {
                            if let Some(idx) = t.automation_lanes.iter().position(|l| l.effect_instance_id == eid && l.param_id == pid) {
                                t.automation_lanes.remove(idx);
                            } else {
                                t.automation_lanes.push(lane.clone());
                            }
                        }
                    }
                    if let Some(track) = app.project.tracks.get_mut(track) {
                        if let Some(idx) = track.automation_lanes.iter().position(|l| l.effect_instance_id == eid && l.param_id == pid) {
                            track.automation_lanes.remove(idx);
                        } else {
                            track.automation_lanes.push(lane);
                        }
                    }
                };

                for &(name, pid) in &[("Volume", PARAM_VOLUME), ("Pan", PARAM_PAN)] {
                    let checked = has_lane(None, pid);
                    let label = if checked { format!("{name}  ✓") } else { name.to_string() };
                    if ui.selectable_label(false, label).clicked() {
                        ui.close_menu();
                        toggle(track_idx, None, pid, name);
                        close = true;
                    }
                }

                if let Ok(tracks) = engine.tracks.lock() {
                    if let Some(track) = tracks.get(track_idx) {
                        for inst in &track.fx_chain {
                            if inst.parameter_info().is_empty() { continue; }
                            ui.menu_button(&inst.name, |ui| {
                                for p in inst.parameter_info() {
                                    let checked = has_lane(Some(inst.id), p.id);
                                    let label = if checked { format!("{}  ✓", p.name) } else { p.name.clone() };
                                    if ui.selectable_label(false, label).clicked() {
                                        ui.close_menu();
                                        toggle(track_idx, Some(inst.id), p.id, &p.name);
                                        close = true;
                                    }
                                }
                            });
                        }
                    }
                }
            });

            ui.separator();
            if ui.add(egui::Button::new("Delete Track").fill(Color32::from_rgb(0x88, 0x22, 0x22))).clicked() {
                app.delete_track(track_idx);
                close = true;
            }

            ui.separator();
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });

    if !close {
        if let Some(inner) = wr {
            if ui.input(|i| i.pointer.primary_clicked()) {
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    if !inner.response.rect.contains(pos) {
                        close = true;
                    }
                }
            }
        }
    }

    if close {
        app.timeline_state.track_context_menu = None;
    }
}

fn handle_clip_context_menu(ui: &Ui, app: &mut HdawApp) {
    let (track_idx, clip_id) = match app.timeline_state.clip_context_menu {
        Some(v) => v,
        None => return,
    };

    let clip_name = app.project.tracks.get(track_idx)
        .and_then(|t| t.clips.iter().find_map(|c| match c {
            crate::project::clip::ClipKind::Audio(a) if a.id == clip_id => Some(a.name.clone()),
            crate::project::clip::ClipKind::Midi(m) if m.id == clip_id => Some(m.name.clone()),
            _ => None,
        }))
        .unwrap_or_default();

    let has_clipboard = app.clipboard.is_some();
    let mut close = false;

    let wr = egui::Window::new(format!("Clip: {clip_name}"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::LEFT_TOP, (0.0, 0.0))
        .show(ui.ctx(), |ui| {
            if ui.button("Copy").clicked() {
                if let Some(track) = app.project.tracks.get(track_idx) {
                    if let Some(c) = track.clips.iter().find(|c| match c {
                        crate::project::clip::ClipKind::Audio(a) => a.id == clip_id,
                        crate::project::clip::ClipKind::Midi(m) => m.id == clip_id,
                    }).cloned() {
                        app.clipboard = Some(c);
                    }
                }
                close = true;
            }
            if has_clipboard {
                if ui.button("Paste").clicked() {
                    if let Some(c) = app.clipboard.clone() {
                        let new_id = uuid::Uuid::new_v4();
                        match &c {
                            crate::project::clip::ClipKind::Audio(a) => {
                                let mut new = a.clone();
                                new.id = new_id;
                                new.position_frames = app.engine.transport.position_frames();
                                if let Some(track) = app.project.tracks.get_mut(track_idx) {
                                    track.add_clip(crate::project::clip::ClipKind::Audio(new));
                                }
                                if let Ok(mut tracks) = app.engine.tracks.lock() {
                                    if let Some(handle) = tracks.get_mut(track_idx) {
                                        let sr = app.engine.transport.sample_rate();
                                        let ch = crate::project::clip_handle::ClipHandle::new(
                                            new_id, (**a.buffer.as_ref().map(|b| b.samples()).unwrap()).clone(),
                                            a.buffer.as_ref().map(|b| b.channels()).unwrap_or(0),
                                            a.buffer.as_ref().map(|b| b.sample_rate()).unwrap_or(sr),
                                        );
                                        ch.set_position(app.engine.transport.position_frames());
                                        handle.add_clip(ch);
                                    }
                                }
                            }
                            crate::project::clip::ClipKind::Midi(m) => {
                                let mut new = m.clone();
                                new.id = new_id;
                                new.position_frames = app.engine.transport.position_frames();
                                if let Some(track) = app.project.tracks.get_mut(track_idx) {
                                    track.add_clip(crate::project::clip::ClipKind::Midi(new));
                                }
                                if let Ok(mut tracks) = app.engine.tracks.lock() {
                                    if let Some(handle) = tracks.get_mut(track_idx) {
                                        let sr = app.engine.transport.sample_rate();
                                        let ch = crate::project::clip_handle::ClipHandle::new_midi(new_id, m.notes.clone(), m.length_frames, sr);
                                        ch.set_position(app.engine.transport.position_frames());
                                        handle.add_clip(ch);
                                    }
                                }
                            }
                        }
                    }
                    close = true;
                }
            }
            ui.separator();
            if ui.button("Duplicate").clicked() {
                app.duplicate_clip(track_idx, clip_id);
                close = true;
            }
            if ui.button("Glue").clicked() {
                // Find adjacent clip after this one
                if let Some(track) = app.project.tracks.get(track_idx) {
                    let mut clips: Vec<(uuid::Uuid, u64)> = track.clips.iter().filter_map(|c| match c {
                        crate::project::clip::ClipKind::Audio(a) => Some((a.id, a.position_frames)),
                        crate::project::clip::ClipKind::Midi(m) => Some((m.id, m.position_frames)),
                    }).collect();
                    clips.sort_by_key(|c| c.1);
                    let pos = clips.iter().position(|c| c.0 == clip_id);
                    if let Some(idx) = pos {
                        if idx + 1 < clips.len() {
                            app.glue_clips(track_idx, clip_id, clips[idx + 1].0);
                        }
                    }
                }
                close = true;
            }
            if ui.button("Delete").clicked() {
                app.timeline_state.selected_clip_id = Some(clip_id);
                app.remove_selected_clip();
                close = true;
            }
            if ui.button("Rename...").clicked() {
                app.clip_rename_text = clip_name;
                close = true;
                // Rename handled inline: show text edit + apply button
            }
            ui.separator();
            if ui.button("Close").clicked() {
                close = true;
            }
        });

    if !close {
        if let Some(inner) = wr {
            if ui.input(|i| i.pointer.primary_clicked()) {
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    if !inner.response.rect.contains(pos) {
                        close = true;
                    }
                }
            }
        }
    }

    // Rename dialog
    if app.clip_rename_text.len() > 0 && app.timeline_state.clip_context_menu.is_some() {
        let mut rename_close = false;
        egui::Window::new("Rename Clip")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    if ui.add(egui::TextEdit::singleline(&mut app.clip_rename_text)
                        .desired_width(200.0)).lost_focus() {
                        // Apply on Enter
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            rename_close = true;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        rename_close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        rename_close = true;
                        app.clip_rename_text = String::new();
                    }
                });
            });
        if rename_close && !app.clip_rename_text.is_empty() {
            if let Some(track) = app.project.tracks.get_mut(track_idx) {
                for clip in track.clips.iter_mut() {
                    match clip {
                        crate::project::clip::ClipKind::Audio(a) if a.id == clip_id => {
                            a.name = app.clip_rename_text.clone();
                        }
                        crate::project::clip::ClipKind::Midi(m) if m.id == clip_id => {
                            m.name = app.clip_rename_text.clone();
                        }
                        _ => {}
                    }
                }
            }
            app.clip_rename_text = String::new();
        }
        if rename_close {
            // Close the rename dialog
        }
    }

    if close {
        app.timeline_state.clip_context_menu = None;
        app.clip_rename_text = String::new();
    }
}

fn handle_ruler_context_menu(ui: &Ui, app: &mut HdawApp) {
    let sr = app.engine.transport.sample_rate();
    let ctx_menu = match app.timeline_state.ruler_context_menu {
        Some(ref cm) => cm.frame,
        None => return,
    };

    let mut close = false;

    let wr = egui::Window::new("Ruler")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::LEFT_TOP, (0.0, 0.0))
        .show(ui.ctx(), |ui| {
            if ui.button("Set Loop Start Here").clicked() {
                let (_, loop_out) = app.engine.transport.load_loop_region();
                app.engine.transport.set_loop_region(ctx_menu, loop_out.max(ctx_menu));
                app.engine.transport.loop_enabled.store(true, Ordering::Release);
                close = true;
            }
            if ui.button("Set Loop End Here").clicked() {
                let (loop_in, _) = app.engine.transport.load_loop_region();
                app.engine.transport.set_loop_region(loop_in.min(ctx_menu), ctx_menu);
                app.engine.transport.loop_enabled.store(true, Ordering::Release);
                close = true;
            }
            ui.separator();
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });

    if !close {
        if let Some(inner) = wr {
            if ui.input(|i| i.pointer.primary_clicked()) {
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    if !inner.response.rect.contains(pos) {
                        close = true;
                    }
                }
            }
        }
    }

    if close {
        app.timeline_state.ruler_context_menu = None;
    }
}

fn handle_zoom_and_scroll(ui: &Ui, response: &Response, rect: &Rect, app: &mut HdawApp, header_width: f32) {
    let is_over = ui.input(|i| i.pointer.hover_pos())
        .map_or(false, |p| p.x > rect.left() + header_width && rect.contains(p));
    if !is_over { return; }
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

fn draw_grid_lines(painter: &egui::Painter, rect: &Rect, state: &TimelineState, header_width: f32, bpm: f64, prefs: &crate::ui::preferences::PreferencesState, tempo_events: &[crate::project::tempo_event::TempoEvent], sample_rate: u32) {
    if !state.snap_enabled { return; }
    let pps = state.pixels_per_second;
    let use_tempo_track = !tempo_events.is_empty();
    let sr = sample_rate as f64;

    let start_secs = state.scroll_x / pps;
    let end_secs = (state.scroll_x + rect.width() as f64) / pps;
    let start_frame = (start_secs * sr).max(0.0) as u64;
    let end_frame = (end_secs * sr) as u64;

    let start_beat = if use_tempo_track {
        crate::project::tempo_event::frames_to_beats(start_frame, tempo_events, sample_rate)
    } else {
        start_secs * (bpm / 60.0)
    };
    let end_beat = if use_tempo_track {
        crate::project::tempo_event::frames_to_beats(end_frame, tempo_events, sample_rate)
    } else {
        end_secs * (bpm / 60.0)
    };

    let effective_bpm = if use_tempo_track { crate::project::tempo_event::tempo_at(tempo_events, start_frame) } else { bpm };
    let step = state.beat_step(effective_bpm, prefs.grid_division);

    let first_tick = (start_beat / step).ceil() * step;
    let base_alpha = (prefs.grid_opacity * 255.0) as u8;

    let mut beat = first_tick;
    while beat <= end_beat {
        let x = if use_tempo_track {
            let f = crate::project::tempo_event::beats_to_frames(beat, tempo_events, sample_rate);
            let secs = f as f64 / sr;
            rect.left() + (secs * pps - state.scroll_x) as f32
        } else {
            let secs = beat * 60.0 / bpm;
            rect.left() + (secs * pps - state.scroll_x) as f32
        };

        if x >= rect.left() + header_width && x <= rect.right() {
            let top_y = rect.top() + RULER_HEIGHT;
            let alpha_mult = if (beat / 4.0).fract().abs() < 0.001 { 1.0 }
                       else if (beat / 1.0).fract().abs() < 0.001 { 0.5 }
                       else { 0.25 };
            let alpha = (base_alpha as f32 * alpha_mult) as u8;
            let grid_color = Color32::from_rgba_premultiplied(80, 80, 80, alpha);
            painter.line_segment(
                [pos2(x, top_y), pos2(x, rect.bottom())],
                egui::Stroke::new(1.0, grid_color),
            );
        }
        beat += step;
    }
}

fn render_tracks(painter: &egui::Painter, rect: &Rect, sr: u32, app: &mut HdawApp, header_width: f32, track_height: f32) {
    let track_count = app.track_ui.len();

    let fx_infos: Vec<track_headers::TrackFxInfo> = if let Ok(tracks) = app.engine.tracks.lock() {
        tracks.iter().map(|t| {
            let inst_idx = t.fx_chain.iter().position(|e| e.has_note_input);
            let instrument_name = inst_idx.map(|i| t.fx_chain[i].name.clone());
            let fx_names: Vec<String> = t.fx_chain.iter()
                .filter(|e| !e.has_note_input)
                .map(|e| e.name.clone())
                .collect();
            track_headers::TrackFxInfo { instrument_name, fx_names }
        }).collect()
    } else {
        vec![track_headers::TrackFxInfo::default(); track_count]
    };

    // Precompute which tracks are hidden by collapsed group ancestors
    let mut hidden_by_collapse = vec![false; track_count];
    for (i, hidden) in hidden_by_collapse.iter_mut().enumerate() {
        let mut cursor = i;
        loop {
            let pg = app.track_ui[cursor].parent_group;
            match pg {
                Some(pid) => {
                    let parent_idx = app.track_ui.iter().position(|t| t.id == pid);
                    match parent_idx {
                        Some(pi) if pi < i || pi != cursor => {
                            if app.track_ui[pi].collapsed {
                                *hidden = true;
                                break;
                            }
                            cursor = pi;
                        }
                        _ => break,
                    }
                }
                None => break,
            }
        }
    }

    let mut visible_idx = 0usize;
    for i in 0..track_count {
        if hidden_by_collapse[i] { continue; }

        let track_y = rect.top() + RULER_HEIGHT + visible_idx as f32 * track_height
            + app.timeline_state.scroll_y as f32;
        visible_idx += 1;

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
        let track_ui = &app.track_ui[i];
        track_headers::draw(painter, &header_rect, track_ui, is_selected, &fx_infos[i]);

        if app.track_ui[i].is_group || app.track_ui[i].is_return {
            let lane_bg = if app.track_ui[i].is_group {
                Color32::from_rgb(0x2a, 0x2a, 0x1a)
            } else {
                Color32::from_rgb(0x2a, 0x1a, 0x2a)
            };
            painter.rect_filled(lane_rect, 0.0, lane_bg);
        } else {
            let lane_bg = Color32::from_rgb(0x22, 0x22, 0x22);
            painter.rect_filled(lane_rect, 0.0, lane_bg);

            let clip_count = app.project.tracks[i].clips.len();
            for ci in 0..clip_count {
                let clip = app.project.tracks[i].clips[ci].clone();
                clips::draw(painter, &lane_rect, &clip, app, sr);
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
    if !app.is_playing() || !app.preferences.follow_playhead { return; }
    let pps = app.timeline_state.pixels_per_second;
    let lane_width = (rect.width() - header_width) as f64;
    let pos_secs = app.position_seconds();
    let playhead_local = pos_secs * pps - app.timeline_state.scroll_x;
    let threshold = lane_width * 0.75;
    if playhead_local > threshold {
        app.timeline_state.scroll_x = pos_secs * pps - threshold;
    }
}
