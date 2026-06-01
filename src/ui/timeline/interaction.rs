use crate::app::HdawApp;
use crate::project::clip::ClipKind;
use crate::ui::timeline::clips;
use crate::ui::timeline::track_headers;
use crate::ui::timeline::{DragMode, LoopDragState, LoopHandle, RULER_HEIGHT};
use egui::{pos2, vec2, Rect, Response, Ui};
use std::sync::atomic::Ordering;

pub fn handle_drag_end_snap(response: &Response, app: &mut HdawApp) {
    if response.drag_stopped() {
        if let Some(drag) = app.timeline_state.drag_state.take() {
            let old_pos = drag.original_position_frames;
            let old_off = drag.original_offset_frames;
            let old_len = drag.original_length_frames;
            let old_fi = drag.original_fade_in;
            let old_fo = drag.original_fade_out;

            match drag.mode {
                DragMode::FadeIn | DragMode::FadeOut => {
                    if let Some(track) = app.project.tracks.get(drag.track_index) {
                        if let Some(ClipKind::Audio(clip)) = track.clips.iter().find(|c| matches!(c, ClipKind::Audio(a) if a.id == drag.clip_id)) {
                            let new_fi = clip.fade_in_frames;
                            let new_fo = clip.fade_out_frames;
                            if old_fi != new_fi || old_fo != new_fo {
                                app.undo_state.push(crate::app::undo::UndoCommand::FadeClip {
                                    track_index: drag.track_index,
                                    clip_id: drag.clip_id,
                                    old_fade_in: old_fi,
                                    old_fade_out: old_fo,
                                    new_fade_in: new_fi,
                                    new_fade_out: new_fo,
                                });
                            }
                        }
                    }
                    return;
                }
                _ => {}
            }

            if app.timeline_state.snap_enabled {
                let sr = app.engine.transport.sample_rate();
                let bpm = app.project.bpm;
                if let Some(track) = app.project.tracks.get_mut(drag.track_index) {
                    if let Some(ClipKind::Audio(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Audio(a) if a.id == drag.clip_id)) {
                        let snapped = app.timeline_state.snap_frames_to_grid(clip.position_frames, sr, bpm, &app.preferences, &app.project.markers);
                        let delta = snapped as i64 - clip.position_frames as i64;
                        if delta != 0 {
                            clip.position_frames = snapped;
                            if let Ok(tracks) = app.engine.tracks.lock() {
                                if let Some(handle) = tracks.get(drag.track_index) {
                                    if let Some(ch) = handle.clips.iter().find(|c| c.clip_id == drag.clip_id) {
                                        let old_p = ch.position_frames.load(Ordering::Acquire);
                                        ch.position_frames.store((old_p as i64 + delta).max(0) as u64, Ordering::Release);
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                if let Some(track) = app.project.tracks.get(drag.track_index) {
                    if let Some(ClipKind::Audio(clip)) = track.clips.iter().find(|c| matches!(c, ClipKind::Audio(a) if a.id == drag.clip_id)) {
                        let new_pos = clip.position_frames;
                        let new_off = clip.offset_frames;
                        let new_len = clip.length_frames;
                        match drag.mode {
                            DragMode::Move => {
                                if old_pos != new_pos {
                                    app.undo_state.push(crate::app::undo::UndoCommand::MoveClip {
                                        track_index: drag.track_index,
                                        clip_id: drag.clip_id,
                                        old_position: old_pos,
                                        new_position: new_pos,
                                    });
                                }
                            }
                            _ => {
                                if old_off != new_off || old_len != new_len {
                                    app.undo_state.push(crate::app::undo::UndoCommand::TrimClip {
                                        track_index: drag.track_index,
                                        clip_id: drag.clip_id,
                                        old_offset: old_off,
                                        old_length: old_len,
                                        new_offset: new_off,
                                        new_length: new_len,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn handle_seek_click(response: &Response, ui: &Ui, rect: &Rect, sr: u32, app: &mut HdawApp, header_width: f32) {
    if response.clicked_by(egui::PointerButton::Primary)
        && app.timeline_state.drag_state.is_none()
        && app.timeline_state.loop_drag.is_none()
    {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            if pos.x > rect.left() + header_width {
                let timeline_x = (pos.x - rect.left() - header_width) as f64
                    + app.timeline_state.scroll_x;
                let time = timeline_x / app.timeline_state.pixels_per_second;
                if time >= 0.0 {
                    let bpm = app.project.bpm;
                    let frame = app.timeline_state.snap_frames_to_grid((time * sr as f64) as u64, sr, bpm, &app.preferences, &app.project.markers);
                    app.seek_requested = true;
                    app.seek_frame = frame;
                    app.timeline_state.selected_clip_id = None;
                }
            }
        }
    }
}

pub fn handle_loop_interaction(response: &Response, ui: &Ui, rect: &Rect, sr: u32, app: &mut HdawApp, header_width: f32) {
    let pps = app.timeline_state.pixels_per_second;
    let scroll_x = app.timeline_state.scroll_x;

    let (loop_in, loop_out) = app.engine.transport.load_loop_region();

    let loop_in_x = rect.left() + (loop_in as f64 / sr as f64 * pps - scroll_x) as f32;
    let loop_out_x = rect.left() + (loop_out as f64 / sr as f64 * pps - scroll_x) as f32;
    let hit_margin = 10.0;

    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
        let in_hit = (pos.x - loop_in_x).abs() <= hit_margin && pos.y <= rect.top() + RULER_HEIGHT;
        let out_hit = (pos.x - loop_out_x).abs() <= hit_margin && pos.y <= rect.top() + RULER_HEIGHT;

        if !app.timeline_state.loop_drag.is_some() {
            if response.clicked_by(egui::PointerButton::Primary) && (in_hit || out_hit) {
                let handle = if in_hit { LoopHandle::In } else { LoopHandle::Out };
                let original = if in_hit { loop_in } else { loop_out };
                app.timeline_state.loop_drag = Some(LoopDragState {
                    handle,
                    drag_start_x: pos.x as f64,
                    original_frame: original,
                });
                return;
            }
        }
    }

    if let Some(drag) = app.timeline_state.loop_drag.as_ref() {
        if response.dragged() {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let timeline_x = (pos.x - rect.left() - header_width) as f64 + scroll_x;
                let time = timeline_x / pps;
                if time >= 0.0 {
                    let frame = (time * sr as f64).round().max(0.0) as u64;
                    match drag.handle {
                        LoopHandle::In => app.engine.transport.store_loop_in(frame),
                        LoopHandle::Out => app.engine.transport.store_loop_out(frame),
                    }
                }
            }
        }

        if response.drag_stopped() {
            app.timeline_state.loop_drag = None;
            app.engine.transport.loop_enabled.store(true, Ordering::Release);
        }
    }
}

pub fn handle_clip_interaction(response: &Response, ui: &Ui, rect: &Rect, app: &mut HdawApp, header_width: f32, track_height: f32) {
    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
        if response.hover_pos().is_some() && pos.x > rect.left() + header_width {
            clips::handle_interaction(response, app, pos, rect, header_width, track_height);
        }
    }
}

pub fn handle_track_header_interaction(response: &Response, ui: &Ui, rect: &Rect, app: &mut HdawApp, header_width: f32, track_height: f32) {
    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
        if response.hover_pos().is_some()
            && pos.x >= rect.left()
            && pos.x <= rect.left() + header_width
        {
            if response.clicked_by(egui::PointerButton::Secondary) {
                let track_count = app.track_ui.len();
                for i in 0..track_count {
                    let track_y = rect.top() + RULER_HEIGHT + i as f32 * track_height
                        + app.timeline_state.scroll_y as f32;
                    let header_rect = Rect::from_min_size(
                        pos2(rect.left(), track_y),
                        vec2(header_width, track_height),
                    );
                    if header_rect.contains(pos) {
                        app.select_track(i);
                        app.timeline_state.track_context_menu = Some(i);
                        break;
                    }
                }
                return;
            }

            let track_count = app.track_ui.len();
            for i in 0..track_count {
                let track_y = rect.top() + RULER_HEIGHT + i as f32 * track_height
                    + app.timeline_state.scroll_y as f32;
                let header_rect = Rect::from_min_size(
                    pos2(rect.left(), track_y),
                    vec2(header_width, track_height),
                );

                if !header_rect.contains(pos) { continue; }

                if response.dragged_by(egui::PointerButton::Primary) {
                    let action = track_headers::hit_test(&header_rect, pos, app.track_ui[i].is_group, app.track_ui[i].is_return);
                    match action {
                        track_headers::HeaderAction::Volume => {
                            let v_rect = track_headers::volume_rect(&header_rect);
                            let val = ((pos.x - v_rect.left()) / v_rect.width()).clamp(0.0, 1.0);
                            app.track_ui[i].volume.store(val.to_bits(), Ordering::Release);
                        }
                        track_headers::HeaderAction::Pan => {
                            let p_rect = track_headers::pan_rect(&header_rect);
                            let val = ((pos.x - p_rect.left()) / p_rect.width()).clamp(0.0, 1.0);
                            app.track_ui[i].pan.store(val.to_bits(), Ordering::Release);
                        }
                        _ => {}
                    }
                }

                if response.clicked() {
                    let action = track_headers::hit_test(&header_rect, pos, app.track_ui[i].is_group, app.track_ui[i].is_return);
                    match action {
                        track_headers::HeaderAction::ToggleMute => {
                            app.toggle_track_mute(i);
                        }
                        track_headers::HeaderAction::ToggleSolo => {
                            app.toggle_track_solo(i);
                        }
                        track_headers::HeaderAction::ToggleArm => {
                            app.toggle_track_arm(i);
                        }
                        track_headers::HeaderAction::ToggleCollapse => {
                            app.track_ui[i].collapsed ^= true;
                        }
                        track_headers::HeaderAction::Select => {
                            app.select_track(i);
                        }
                        _ => {}
                    }
                }
                break;
            }
        }
    }
}


