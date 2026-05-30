use crate::app::HdawApp;
use crate::project::clip::{AudioClip, ClipKind};
use crate::project::midi_clip::MidiClip;

use crate::ui::timeline::{DragMode, DragState, TimelineState, CLIP_CORNER_RADIUS, RULER_HEIGHT};
use egui::{pos2, vec2, Color32, Pos2, Rect, Response, Stroke};

pub fn draw(
    painter: &egui::Painter,
    lane_rect: &Rect,
    clip: &ClipKind,
    state: &TimelineState,
    sample_rate: u32,
) {
    match clip {
        ClipKind::Audio(audio_clip) => draw_audio(painter, lane_rect, audio_clip, state, sample_rate),
        ClipKind::Midi(midi_clip) => draw_midi(painter, lane_rect, midi_clip, state, sample_rate),
    }
}

fn draw_audio(
    painter: &egui::Painter,
    lane_rect: &Rect,
    clip: &AudioClip,
    state: &TimelineState,
    sample_rate: u32,
) {
    let sr = sample_rate as f64;
    let pps = state.pixels_per_second;
    let scroll_x = state.scroll_x;

    let clip_left = (clip.position_frames as f64 / sr) * pps - scroll_x;
    let clip_width = ((clip.length_frames - clip.offset_frames) as f64 / sr) * pps;
    let left_pixel = (lane_rect.left() + clip_left as f32).max(lane_rect.left());
    let top = lane_rect.top() + 2.0;
    let height = lane_rect.height() - 4.0;

    if left_pixel > lane_rect.right() || (clip_left as f32 + clip_width as f32) < 0.0 {
        return;
    }

    let available_w = (clip_width as f32).min(lane_rect.right() - left_pixel).max(1.0);
    let clip_rect = Rect::from_min_size(pos2(left_pixel, top), vec2(available_w, height));

    let is_selected = state.selected_clip_id == Some(clip.id);
    let bg = Color32::from_rgb(0x2a, 0x3a, 0x2a);
    let border = if is_selected {
        Color32::from_rgb(0x64, 0xb5, 0xf6)
    } else {
        Color32::from_rgb(0x3a, 0x5a, 0x3a)
    };
    painter.rect_filled(clip_rect, CLIP_CORNER_RADIUS, bg);
    painter.rect_stroke(clip_rect, CLIP_CORNER_RADIUS, Stroke::new(1.0, border));

    if available_w > 4.0 {
        if let Some(ref waveform) = clip.waveform_peaks {
            let peaks = waveform.peaks.as_ref();
            if !peaks.is_empty() {
                let step = available_w / peaks.len() as f32;
                let mid_y = clip_rect.center().y;
                let amp = (clip_rect.height() / 2.0 - 2.0).max(1.0);
                let wave_color = Color32::from_rgba_premultiplied(0x8b, 0xc3, 0x4a, 180);
                for (pi, p) in peaks.iter().enumerate() {
                    let px = left_pixel + pi as f32 * step;
                    if px < lane_rect.left() || px > lane_rect.right() {
                        continue;
                    }
                    let y1 = mid_y - p.max.abs() * amp;
                    let y2 = mid_y + p.min.abs() * amp;
                    if (y2 - y1) > 0.5 || p.max.abs() > 0.001 {
                        painter.line_segment(
                            [pos2(px, y1), pos2(px, y2)],
                            Stroke::new(1.0, wave_color),
                        );
                    }
                }
            }
        }
    }

    if available_w > 40.0 {
        let small_font = egui::FontId::proportional(10.0);
        painter.text(
            pos2(clip_rect.left() + 4.0, clip_rect.top() + 2.0),
            egui::Align2::LEFT_TOP,
            &clip.name,
            small_font,
            Color32::from_gray(200),
        );
    }
}

fn draw_midi(
    painter: &egui::Painter,
    lane_rect: &Rect,
    clip: &MidiClip,
    state: &TimelineState,
    sample_rate: u32,
) {
    let sr = sample_rate as f64;
    let pps = state.pixels_per_second;
    let scroll_x = state.scroll_x;

    let clip_left = (clip.position_frames as f64 / sr) * pps - scroll_x;
    let clip_width = (clip.length_frames as f64 / sr) * pps;
    let left_pixel = (lane_rect.left() + clip_left as f32).max(lane_rect.left());
    let top = lane_rect.top() + 2.0;
    let height = lane_rect.height() - 4.0;

    if left_pixel > lane_rect.right() || (clip_left as f32 + clip_width as f32) < 0.0 {
        return;
    }

    let available_w = (clip_width as f32).min(lane_rect.right() - left_pixel).max(1.0);
    let clip_rect = Rect::from_min_size(pos2(left_pixel, top), vec2(available_w, height));

    let is_selected = state.selected_clip_id == Some(clip.id);
    let c = clip.color;
    let bg = Color32::from_rgb(c[0], c[1], c[2]);
    let border = if is_selected {
        Color32::from_rgb(0x64, 0xb5, 0xf6)
    } else {
        Color32::from_rgb(
            c[0].saturating_add(0x20),
            c[1].saturating_add(0x20),
            c[2].saturating_add(0x20),
        )
    };
    painter.rect_filled(clip_rect, CLIP_CORNER_RADIUS, bg);
    painter.rect_stroke(clip_rect, CLIP_CORNER_RADIUS, Stroke::new(1.0, border));

    if available_w > 40.0 {
        let small_font = egui::FontId::proportional(10.0);
        let label = format!("\u{266b} {} notes", clip.notes.len());
        painter.text(
            pos2(clip_rect.left() + 4.0, clip_rect.center().y),
            egui::Align2::LEFT_CENTER,
            &clip.name,
            small_font.clone(),
            Color32::WHITE,
        );
        painter.text(
            pos2(clip_rect.right() - 4.0, clip_rect.center().y),
            egui::Align2::RIGHT_CENTER,
            &label,
            small_font,
            Color32::from_gray(200),
        );
    }
}

pub fn handle_interaction(
    response: &Response,
    app: &mut HdawApp,
    pos: Pos2,
    rect: &Rect,
    header_width: f32,
    track_height: f32,
) {
    let sr_f = app.engine.transport.sample_rate() as f64;
    let pps = app.timeline_state.pixels_per_second;
    let scroll = app.timeline_state.scroll_x;

    for (track_idx, track) in app.project.tracks.iter().enumerate() {
        let track_y = rect.top() + RULER_HEIGHT + track_idx as f32 * track_height
            + app.timeline_state.scroll_y as f32;
        if pos.y < track_y || pos.y > track_y + track_height {
            continue;
        }

        for clip_kind in &track.clips {
            let (clip_id, position_frames, length_frames) = match clip_kind {
                ClipKind::Audio(a) => (a.id, a.position_frames, a.length_frames),
                ClipKind::Midi(m) => (m.id, m.position_frames, m.length_frames),
            };

            let clip_left = (position_frames as f64 / sr_f) * pps - scroll;

            let clip_width = match clip_kind {
                ClipKind::Audio(a) => ((a.length_frames - a.offset_frames) as f64 / sr_f) * pps,
                ClipKind::Midi(_) => (length_frames as f64 / sr_f) * pps,
            };

            let left_pixel = (rect.left() + header_width) as f64 + clip_left;
            let right_pixel = left_pixel + clip_width;
            let edge = 6.0f64;

            if response.dragged() && app.timeline_state.drag_state.is_some() {
                if let Some(ref drag) = app.timeline_state.drag_state {
                    if drag.clip_id == clip_id {
                        let delta_x = pos.x as f64 - drag.drag_start_x;
                        let delta_frames = (delta_x / pps * sr_f) as i64;
                        match drag.mode {
                            DragMode::Move => {
                                let new_pos = (drag.original_position_frames as i64 + delta_frames)
                                    .max(0) as u64;
                                app.update_clip_position(track_idx, clip_id, new_pos);
                            }
                            DragMode::TrimLeft => {
                                let new_off = (drag.original_offset_frames as i64 + delta_frames)
                                    .max(0) as u64;
                                let new_len = drag.original_length_frames.saturating_sub(
                                    (delta_frames.max(0) as u64).min(drag.original_length_frames),
                                );
                                app.update_clip_trim(track_idx, clip_id, None, Some(new_off), Some(new_len));
                            }
                            DragMode::TrimRight => {
                                let new_len = (drag.original_length_frames as i64 + delta_frames)
                                    .max(1) as u64;
                                app.update_clip_trim(track_idx, clip_id, None, None, Some(new_len));
                            }
                        }
                        return;
                    }
                }
                continue;
            }

            if response.dragged() && app.timeline_state.drag_state.is_none() {
                let local_x = pos.x as f64;
                if (local_x - left_pixel).abs() < edge {
                    app.timeline_state.drag_state = Some(DragState {
                        clip_id,
                        track_index: track_idx,
                        drag_start_x: pos.x as f64,
                        original_position_frames: position_frames,
                        original_offset_frames: 0,
                        original_length_frames: length_frames,
                        mode: DragMode::TrimLeft,
                    });
                    return;
                }
                if (local_x - right_pixel).abs() < edge {
                    app.timeline_state.drag_state = Some(DragState {
                        clip_id,
                        track_index: track_idx,
                        drag_start_x: pos.x as f64,
                        original_position_frames: position_frames,
                        original_offset_frames: 0,
                        original_length_frames: length_frames,
                        mode: DragMode::TrimRight,
                    });
                    return;
                }
                if local_x >= left_pixel && local_x <= right_pixel {
                    app.timeline_state.drag_state = Some(DragState {
                        clip_id,
                        track_index: track_idx,
                        drag_start_x: pos.x as f64,
                        original_position_frames: position_frames,
                        original_offset_frames: 0,
                        original_length_frames: length_frames,
                        mode: DragMode::Move,
                    });
                    app.timeline_state.selected_clip_id = Some(clip_id);
                    return;
                }
            }

            if response.clicked() && pos.x as f64 >= left_pixel && pos.x as f64 <= right_pixel {
                app.timeline_state.selected_clip_id = Some(clip_id);
                return;
            }

            if response.double_clicked() && pos.x as f64 >= left_pixel && pos.x as f64 <= right_pixel {
                if matches!(clip_kind, ClipKind::Midi(_)) {
                    app.show_piano_roll = true;
                    app.editing_midi_clip_id = Some(clip_id);
                }
                return;
            }
        }
    }

    if response.clicked() {
        app.timeline_state.selected_clip_id = None;
    }
}
