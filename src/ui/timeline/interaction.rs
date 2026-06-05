use crate::app::HdawApp;
use crate::project::clip::ClipKind;
use crate::ui::timeline::clips;
use crate::ui::timeline::track_headers;
use crate::ui::timeline::{
    compute_track_y_positions,
    DragMode, LoopDragState, LoopHandle, VelocityDragState,
    LANE_ROW_HEIGHT, VELOCITY_LANE_HEIGHT, RULER_HEIGHT,
};
use egui::{pos2, vec2, Rect, Response, Ui};
use std::sync::atomic::Ordering;

fn find_clip_pos(track: &crate::project::track::Track, clip_id: uuid::Uuid) -> Option<&u64> {
    for c in &track.clips {
        match c {
            ClipKind::Audio(a) if a.id == clip_id => return Some(&a.position_frames),
            ClipKind::Midi(m) if m.id == clip_id => return Some(&m.position_frames),
            _ => {}
        }
    }
    None
}

fn find_clip_pos_mut(track: &mut crate::project::track::Track, clip_id: uuid::Uuid) -> Option<&mut u64> {
    for c in track.clips.iter_mut() {
        match c {
            ClipKind::Audio(a) if a.id == clip_id => return Some(&mut a.position_frames),
            ClipKind::Midi(m) if m.id == clip_id => return Some(&mut m.position_frames),
            _ => {}
        }
    }
    None
}

fn find_audio_clip_for_fade(track: &crate::project::track::Track, clip_id: uuid::Uuid) -> Option<(&u64, &u64)> {
    for c in &track.clips {
        if let ClipKind::Audio(a) = c {
            if a.id == clip_id {
                return Some((&a.fade_in_frames, &a.fade_out_frames));
            }
        }
    }
    None
}

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
                        if let Some((fi, fo)) = find_audio_clip_for_fade(track, drag.clip_id) {
                            let new_fi = *fi;
                            let new_fo = *fo;
                            if old_fi != new_fi || old_fo != new_fo {
                                app.undo_service.push(crate::app::undo::UndoCommand::FadeClip {
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
                DragMode::Stretch => {
                    // On stretch end, compute ratio and sync to both models
                    if let Some(track) = app.project.tracks.get_mut(drag.track_index) {
                        let new_len = track.clips.iter().find_map(|c| match c {
                            crate::project::clip::ClipKind::Audio(a) if a.id == drag.clip_id => Some(a.length_frames),
                            _ => None,
                        });
                        if let Some(nlen) = new_len {
                            if old_len > 0 {
                                let ratio = nlen as f32 / old_len as f32;
                                let ratio = ratio.clamp(0.25, 4.0);
                                if let Some(ClipKind::Audio(a)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Audio(a) if a.id == drag.clip_id)) {
                                    a.stretch_ratio = ratio;
                                }
                                if let Ok(mut tracks) = app.engine.tracks.lock() {
                                    if let Some(handle) = tracks.get_mut(drag.track_index) {
                                        if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == drag.clip_id) {
                                            ch.stretch_ratio.store(ratio.to_bits(), Ordering::Release);
                                        }
                                    }
                                }
                                app.undo_service.push(crate::app::undo::UndoCommand::TrimClip {
                                    track_index: drag.track_index,
                                    clip_id: drag.clip_id,
                                    old_offset: drag.original_offset_frames,
                                    old_length: drag.original_length_frames,
                                    new_offset: 0,
                                    new_length: nlen,
                                });
                            }
                        }
                    }
                    return;
                }
                _ => {}
            }

            // Detect cross-track move
            let is_cross_track = drag.mode == DragMode::Move && drag.track_index != drag.original_track_index;

            if is_cross_track {
                // Cross-track: move clip to new track
                let sr = app.engine.transport.sample_rate();
                let bpm = app.project.bpm;
                let mut new_pos = old_pos;
                // Find current position from project model
                if let Some(track) = app.project.tracks.get(drag.original_track_index) {
                    if let Some(pos_ref) = find_clip_pos(track, drag.clip_id) {
                        new_pos = *pos_ref;
                    }
                }
                if app.timeline_state.snap_enabled {
                    new_pos = app.timeline_state.snap_frames_to_grid(new_pos, sr, bpm, &app.preferences, &app.project.markers);
                }
                let captured_clip = app.project.tracks.get(drag.original_track_index)
                    .and_then(|t| t.clips.iter().find(|c| match c {
                        ClipKind::Audio(a) => a.id == drag.clip_id,
                        ClipKind::Midi(m) => m.id == drag.clip_id,
                    }).cloned());
                app.move_clip_to_track(drag.clip_id, drag.original_track_index, drag.track_index, new_pos);
                if old_pos != new_pos || drag.original_track_index != drag.track_index {
                    if let Some(clip) = captured_clip {
                        app.undo_service.push(crate::app::undo::UndoCommand::MoveClipToTrack {
                            clip_id: drag.clip_id,
                            old_track_index: drag.original_track_index,
                            new_track_index: drag.track_index,
                            old_position: old_pos,
                            new_position: new_pos,
                            clip,
                        });
                    }
                }
            } else if app.timeline_state.snap_enabled {
                let sr = app.engine.transport.sample_rate();
                let bpm = app.project.bpm;
                if let Some(track) = app.project.tracks.get_mut(drag.track_index) {
                    if let Some(pos_ref) = find_clip_pos_mut(track, drag.clip_id) {
                        let snapped = app.timeline_state.snap_frames_to_grid(*pos_ref, sr, bpm, &app.preferences, &app.project.markers);
                        let delta = snapped as i64 - *pos_ref as i64;
                        if delta != 0 {
                            let new_pos = (*pos_ref as i64 + delta).max(0) as u64;
                            *pos_ref = new_pos;
                            if let Ok(tracks) = app.engine.tracks.lock() {
                                if let Some(handle) = tracks.get(drag.track_index) {
                                    if let Some(ch) = handle.clips.iter().find(|c| c.clip_id == drag.clip_id) {
                                        ch.position_frames.store(new_pos, Ordering::Release);
                                    }
                                }
                            }
                            if old_pos != new_pos {
                                app.undo_service.push(crate::app::undo::UndoCommand::MoveClip {
                                    track_index: drag.track_index,
                                    clip_id: drag.clip_id,
                                    old_position: old_pos,
                                    new_position: new_pos,
                                });
                            }
                        }
                    }
                }
            } else {
                if let Some(track) = app.project.tracks.get(drag.track_index) {
                    if let Some(pos_ref) = find_clip_pos(track, drag.clip_id) {
                        let new_pos = *pos_ref;
                        match drag.mode {
                            DragMode::Move => {
                                if old_pos != new_pos {
                                    app.undo_service.push(crate::app::undo::UndoCommand::MoveClip {
                                        track_index: drag.track_index,
                                        clip_id: drag.clip_id,
                                        old_position: old_pos,
                                        new_position: new_pos,
                                    });
                                }
                            }
                            _ => {
                                if let Some(ClipKind::Audio(clip)) = track.clips.iter().find(|c| matches!(c, ClipKind::Audio(a) if a.id == drag.clip_id)) {
                                    if old_off != clip.offset_frames || old_len != clip.length_frames {
                                        app.undo_service.push(crate::app::undo::UndoCommand::TrimClip {
                                            track_index: drag.track_index,
                                            clip_id: drag.clip_id,
                                            old_offset: old_off,
                                            old_length: old_len,
                                            new_offset: clip.offset_frames,
                                            new_length: clip.length_frames,
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

        if app.timeline_state.loop_drag.is_none()
            && response.clicked_by(egui::PointerButton::Primary)
            && (in_hit || out_hit)
        {
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

pub fn handle_velocity_lane_interaction(response: &Response, ui: &Ui, rect: &Rect, app: &mut HdawApp, header_width: f32, track_height: f32) {
    let Some(track_idx) = app.expanded_track else { return };
    let Some(track) = app.project.tracks.get(track_idx) else { return };
    if !track.clips.iter().any(|c| matches!(c, ClipKind::Midi(_))) { return; }

    let track_ys = compute_track_y_positions(rect, app, track_height);
    let Some(track_y) = track_ys.get(track_idx).copied().flatten() else { return };
    let num_lanes = track.automation_lanes.len().max(1) as f32;
    let vel_y = track_y + track_height + num_lanes * LANE_ROW_HEIGHT;
    let vel_rect = Rect::from_min_size(
        pos2(rect.left() + header_width, vel_y),
        vec2((rect.width() - header_width).max(0.0), VELOCITY_LANE_HEIGHT),
    );

    let pos = match ui.input(|i| i.pointer.interact_pos()) {
        Some(p) if vel_rect.contains(p) && response.hover_pos().is_some() => p,
        _ => {
            if response.drag_stopped() {
                if let Some(vd) = app.timeline_state.velocity_drag.take() {
                    let new_note = app.project.tracks.get(vd.track_index)
                        .and_then(|t| t.clips.iter().find_map(|c| match c {
                            ClipKind::Midi(m) if m.id == vd.clip_id => m.notes.get(vd.note_index).cloned(),
                            _ => None,
                        }));
                    if let Some(new_note) = new_note {
                        if new_note.velocity != vd.old_note.velocity {
                            app.undo_service.push(crate::app::undo::UndoCommand::UpdateMidiNote {
                                track_index: vd.track_index,
                                clip_id: vd.clip_id,
                                note_index: vd.note_index,
                                old_note: vd.old_note,
                                new_note,
                            });
                        }
                    }
                }
            }
            return;
        }
    };

    let sr_f = app.engine.transport.sample_rate() as f64;
    let pps = app.timeline_state.pixels_per_second;
    let scroll_x = app.timeline_state.scroll_x;

    let timeline_frame = ((pos.x - vel_rect.left()) as f64 + scroll_x) / pps * sr_f;
    let timeline_frame = timeline_frame as u64;

    // Find the MIDI note closest to click position
    let mut best: Option<(usize, usize, u64, i64)> = None;
    for (ci, clip) in track.clips.iter().enumerate() {
        let ClipKind::Midi(m) = clip else { continue };
        for (ni, note) in m.notes.iter().enumerate() {
            let nf = m.position_frames + note.start_frame;
            let dist = (nf as i64 - timeline_frame as i64).abs();
            if best.map_or(true, |(_, _, _, bd)| dist < bd) {
                best = Some((ci, ni, nf, dist));
            }
        }
    }

    let Some((ci, ni, _nf, dist)) = best else { return };
    let hit_threshold = (sr_f as f64 / pps * 4.0) as u64;
    if dist as u64 > hit_threshold { return; }

    // Find the MIDI clip and note in project model
    let Some((clip_id, clip)) = track.clips.iter().enumerate().find_map(|(idx, c)| match c {
        ClipKind::Midi(m) if idx == ci && m.notes.len() > ni => Some((m.id, c)),
        _ => None,
    }) else { return };

    let ClipKind::Midi(midi_clip) = clip else { return };

    // Map Y to velocity
    let vel_frac = ((vel_rect.bottom() - pos.y) / vel_rect.height()).clamp(0.0, 1.0);
    let new_velocity = (vel_frac * 127.0).round() as u8;

    if response.dragged() && app.timeline_state.velocity_drag.is_some() {
        // Update velocity during drag (no undo push)
        if let Some(track) = app.project.tracks.get_mut(track_idx) {
            if let Some(ClipKind::Midi(m)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(mc) if mc.id == clip_id)) {
                if ni < m.notes.len() {
                    m.notes[ni].velocity = new_velocity;
                }
            }
        }
        if let Ok(mut tracks) = app.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_idx) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    if ni < ch.midi_notes.len() {
                        ch.midi_notes[ni].velocity = new_velocity;
                    }
                }
            }
        }
        return;
    }

    if response.clicked_by(egui::PointerButton::Primary) && !response.dragged() {
        let old_note = midi_clip.notes[ni].clone();
        app.timeline_state.velocity_drag = Some(VelocityDragState {
            track_index: track_idx,
            clip_id,
            note_index: ni,
            old_note,
        });
        if let Some(track) = app.project.tracks.get_mut(track_idx) {
            if let Some(ClipKind::Midi(m)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(mc) if mc.id == clip_id)) {
                if ni < m.notes.len() {
                    m.notes[ni].velocity = new_velocity;
                }
            }
        }
        if let Ok(mut tracks) = app.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_idx) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    if ni < ch.midi_notes.len() {
                        ch.midi_notes[ni].velocity = new_velocity;
                    }
                }
            }
        }
        return;
    }
}

pub fn handle_track_header_interaction(response: &Response, ui: &Ui, rect: &Rect, app: &mut HdawApp, header_width: f32, track_height: f32) {
    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
        if response.hover_pos().is_some()
            && pos.x >= rect.left()
            && pos.x <= rect.left() + header_width
        {
            let track_ys: Vec<Option<f32>> = compute_track_y_positions(rect, app, track_height);

            if response.clicked_by(egui::PointerButton::Secondary) {
                let track_count = app.track_ui.len();
                for i in 0..track_count {
                    let Some(track_y) = track_ys[i] else { continue; };
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
                let Some(track_y) = track_ys[i] else { continue; };
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
                        track_headers::HeaderAction::ToggleExpand => {
                            app.expanded_track = if app.expanded_track == Some(i) { None } else { Some(i) };
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


