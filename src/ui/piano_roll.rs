use crate::app::HdawApp;
use crate::project::clip::ClipKind;
use crate::project::midi_note::MidiNote;
use crate::ui::piano_roll_state::{ControllerLane, PianoRollDragTarget};
use egui::{pos2, vec2, Color32, Vec2};

const VEL_LANE_HEIGHT: f32 = 60.0;
const NOTE_NAME_WIDTH: f32 = 48.0;
const NOTE_BAR_HEIGHT_RATIO: f32 = 0.8;
const NOTE_NAME_FONT_SIZE: f32 = 9.0;
const LANE_LABEL_FONT_SIZE: f32 = 9.0;
const CC_CIRCLE_RADIUS: f32 = 3.0;
const CC_HIT_RADIUS: f32 = 6.0;

static NOTE_NAMES: &[&str; 12] = &["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

fn note_to_name(n: u8) -> String {
    let octave = (n / 12) as i32 - 1;
    format!("{}{}", NOTE_NAMES[(n % 12) as usize], octave)
}

fn is_white_key(n: u8) -> bool {
    matches!(n % 12, 0 | 2 | 4 | 5 | 7 | 9 | 11)
}

pub fn render(ctx: &egui::Context, app: &mut HdawApp) {
    if !app.show_piano_roll {
        return;
    }

    let clip_id = match app.editing_midi_clip_id {
        Some(id) => id,
        None => {
            app.show_piano_roll = false;
            return;
        }
    };

    let sr = app.engine.transport.sample_rate();
    let bpm = app.project.bpm;
    
    // Use piano_roll_state for zoom and scroll
    let pps = app.piano_roll_state.zoom_x;
    let row_height = app.piano_roll_state.zoom_y as f32;

    let clip_data = app.project.tracks.iter().enumerate().find_map(|(ti, t)| {
        t.clips.iter().find_map(|c| match c {
            ClipKind::Midi(m) if m.id == clip_id => Some((ti, m.clone())),
            _ => None,
        })
    });

    let (track_idx, clip) = match clip_data {
        Some(d) => d,
        None => {
            app.show_piano_roll = false;
            return;
        }
    };

    let min_note = app.preferences.piano_roll_min_note;
    let max_note = app.preferences.piano_roll_max_note;
    let note_bar_height = row_height * NOTE_BAR_HEIGHT_RATIO;
    let num_rows = (max_note.saturating_sub(min_note) as usize).max(1) + 1;

    let mut show = app.show_piano_roll;
    let avail = ctx.available_rect();
    let init_w = (avail.width() * 0.8).max(400.0);
    let init_h = (avail.height() * 0.7).max(300.0);
    egui::Window::new(format!("Piano Roll - {}", clip.name))
        .id("piano_roll".into())
        .open(&mut show)
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(init_w, init_h))
        .show(ctx, |ui| {
            // Handle scroll and zoom — only when pointer is over this window
            let is_over = ui.input(|i| i.pointer.hover_pos())
                .is_some_and(|p| ui.clip_rect().contains(p));
            if is_over {
                let mw_delta = ui.input(|i| i.raw_scroll_delta);
                let modifiers = ui.input(|i| i.modifiers);
                
                if mw_delta.y != 0.0 {
                    if modifiers.ctrl {
                        let factor = 1.0 + mw_delta.y as f64 * 0.003;
                        app.piano_roll_state.zoom_x = (app.piano_roll_state.zoom_x * factor).clamp(5.0, 2000.0);
                    } else if modifiers.alt {
                        let factor = 1.0 + mw_delta.y as f64 * 0.003;
                        app.piano_roll_state.zoom_y = (app.piano_roll_state.zoom_y * factor).clamp(8.0, 100.0);
                    } else if modifiers.shift {
                        app.piano_roll_state.scroll_x -= mw_delta.y as f64 * 2.0;
                    } else {
                        app.piano_roll_state.scroll_y -= mw_delta.y as f64 * 2.0;
                    }
                }
                if mw_delta.x != 0.0 {
                     app.piano_roll_state.scroll_x -= mw_delta.x as f64 * 2.0;
                }
            }

            // Note length selector toolbar
            let note_lengths: &[(f64, &str)] = &[
                (4.0, "1/1"),
                (2.0, "1/2"),
                (1.0, "1/4"),
                (0.5, "1/8"),
                (0.25, "1/16"),
                (0.125, "1/32"),
            ];
            ui.horizontal_wrapped(|ui| {
                ui.label("Note:");
                for &(val, label) in note_lengths {
                    let selected = (app.piano_roll_state.note_length - val).abs() < 0.001;
                    if ui.selectable_label(selected, label).clicked() {
                        app.piano_roll_state.note_length = val;
                    }
                }
                ui.separator();
                ui.label("Lane:");
                let mode = app.piano_roll_state.controller_lane;
                let mut new_mode = mode;
                if ui.selectable_label(mode == ControllerLane::None, "Off").clicked() {
                    new_mode = ControllerLane::None;
                }
                if ui.selectable_label(mode == ControllerLane::Velocity, "Vel").clicked() {
                    new_mode = ControllerLane::Velocity;
                }
                if ui.selectable_label(mode == ControllerLane::ReleaseVelocity, "Rel").clicked() {
                    new_mode = ControllerLane::ReleaseVelocity;
                }
                {
                    let is_cc = matches!(mode, ControllerLane::Cc(_));
                    if ui.selectable_label(is_cc, "CC").clicked() {
                        let cur = match mode { ControllerLane::Cc(n) => n, _ => 1 };
                        new_mode = if is_cc { ControllerLane::None } else { ControllerLane::Cc(cur) };
                    }
                }
                app.piano_roll_state.controller_lane = new_mode;
                if matches!(app.piano_roll_state.controller_lane, ControllerLane::Cc(_)) {
                    let common_cc: &[(u8, &str)] = &[
                        (1, "1 Mod"), (7, "7 Vol"), (10, "10 Pan"), (11, "11 Exp"), (64, "64 Sus"),
                    ];
                    for &(n, label) in common_cc {
                        let selected = match app.piano_roll_state.controller_lane {
                            ControllerLane::Cc(v) => v == n,
                            _ => false,
                        };
                        if ui.selectable_label(selected, label).clicked() {
                            app.piano_roll_state.controller_lane = ControllerLane::Cc(n);
                            app.piano_roll_state.cc_number = n;
                        }
                    }
                    let mut cc_str = app.piano_roll_state.cc_number.to_string();
                    if ui.add(egui::TextEdit::singleline(&mut cc_str).desired_width(24.0)).changed() {
                        if let Ok(n) = cc_str.parse::<u8>() {
                            if n <= 127 {
                                app.piano_roll_state.controller_lane = ControllerLane::Cc(n);
                                app.piano_roll_state.cc_number = n;
                            }
                        }
                    }
                }
            });

            let has_lane = app.piano_roll_state.controller_lane != ControllerLane::None;
            let available = ui.available_size();
            let lane_h = if has_lane { VEL_LANE_HEIGHT } else { 0.0 };
            let grid_avail = vec2(available.x, (available.y - lane_h).max(0.0));
            let grid_area_w = grid_avail.x - NOTE_NAME_WIDTH;
            if grid_area_w <= 0.0 {
                return;
            }

            let (response, painter) = ui.allocate_painter(grid_avail, egui::Sense::click_and_drag());
            let origin = response.rect.left_top();

            let note_names_x = origin.x;
            let grid_x = origin.x + NOTE_NAME_WIDTH;
            let bottom = response.rect.bottom();
            let right = response.rect.right();

            // 1. Draw horizontal row backgrounds
            for i in 0..num_rows {
                let note = max_note - i as u8;
                let y = origin.y + i as f32 * row_height - app.piano_roll_state.scroll_y as f32;
                if y + row_height < origin.y { continue; }
                if y > bottom { break; }

                let row_bg = if is_white_key(note) {
                    Color32::from_rgb(0x2c, 0x2c, 0x2c)
                } else {
                    Color32::from_rgb(0x22, 0x22, 0x22)
                };

                painter.rect_filled(
                    egui::Rect::from_min_max(pos2(grid_x, y), pos2(right, (y + row_height).min(bottom))),
                    0.0,
                    row_bg,
                );
            }

            // 2. Draw vertical grid lines
            let bps = bpm / 60.0;
            let pixels_per_beat = pps / bps;
            let step = app.timeline_state.beat_step(bpm, app.preferences.grid_division);
            
            // Grid covers visible area + buffer
            let start_beat = (app.piano_roll_state.scroll_x / pixels_per_beat).floor().max(0.0);
            let visible_beats = (grid_area_w as f64 / pixels_per_beat).ceil();
            let end_beat = start_beat + visible_beats + 4.0; 

            let base_alpha = (app.preferences.grid_opacity * 255.0) as u8;

            let mut beat = (start_beat / step).floor() * step;
            while beat <= end_beat {
                let x = grid_x + (beat * pixels_per_beat) as f32 - app.piano_roll_state.scroll_x as f32;
                if x >= grid_x && x <= right {
                    let is_bar = (beat / 4.0).fract().abs() < 0.001;
                    let is_beat = (beat / 1.0).fract().abs() < 0.001;
                    let alpha_mult = if is_bar { 1.0 } else if is_beat { 0.5 } else { 0.2 };
                    let alpha = (base_alpha as f32 * alpha_mult) as u8;
                    
                    painter.line_segment(
                        [pos2(x, origin.y), pos2(x, bottom)],
                        egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(100, 100, 120, alpha)),
                    );
                }
                beat += step;
            }

            // 3. Draw horizontal separators and keys
            for i in 0..num_rows {
                let note = max_note - i as u8;
                let y = origin.y + i as f32 * row_height - app.piano_roll_state.scroll_y as f32;
                if y + row_height < origin.y { continue; }
                if y > bottom { break; }

                // Horizontal grid line
                painter.line_segment(
                    [pos2(grid_x, y + row_height), pos2(right, y + row_height)],
                    egui::Stroke::new(1.0, Color32::from_rgb(0x1a, 0x1a, 0x1a)),
                );

                // Key visual feedback
                let is_note_on = app.is_playing() && {
                    let cur_f = (app.position_seconds() * sr as f64) as u64;
                    clip.notes.iter().any(|n| {
                        n.pitch == note && cur_f >= clip.position_frames + n.start_frame && cur_f < clip.position_frames + n.start_frame + n.duration
                    })
                };

                let key_bg = if is_note_on {
                    Color32::from_rgb(0x33, 0xcc, 0x33)
                } else if !is_white_key(note) {
                    Color32::from_rgb(0x11, 0x11, 0x11) // Black keys
                } else if note.is_multiple_of(12) {
                    Color32::from_rgb(0x3a, 0x3a, 0x3a) // C highlights
                } else {
                    Color32::from_rgb(0xdd, 0xdd, 0xdd) // White keys
                };

                painter.rect_filled(
                    egui::Rect::from_min_size(pos2(note_names_x, y), vec2(NOTE_NAME_WIDTH, row_height)),
                    0.0,
                    key_bg,
                );

                if is_white_key(note) || note.is_multiple_of(12) {
                    let text_color = if is_note_on { Color32::WHITE } else if !is_white_key(note) { Color32::from_gray(180) } else { Color32::from_gray(40) };
                    painter.text(
                        pos2(note_names_x + 4.0, y + row_height / 2.0),
                        egui::Align2::LEFT_CENTER,
                        note_to_name(note),
                        egui::FontId::proportional(NOTE_NAME_FONT_SIZE),
                        text_color,
                    );
                }
            }

            // Draw playhead
            let current_pos = app.position_seconds();
            let play_x = grid_x + (current_pos * pps) as f32 - app.piano_roll_state.scroll_x as f32;
            if play_x >= grid_x && play_x <= response.rect.right() {
                painter.line_segment(
                    [pos2(play_x, origin.y), pos2(play_x, bottom)],
                    egui::Stroke::new(2.0, Color32::from_rgb(0xff, 0xcc, 0x33)),
                );

                if app.preferences.piano_roll_follow_playhead && app.is_playing() {
                    let threshold = grid_area_w as f64 * 0.75;
                    let local_play_x = current_pos * pps - app.piano_roll_state.scroll_x;
                    if local_play_x > threshold {
                        app.piano_roll_state.scroll_x = current_pos * pps - threshold;
                    }
                }
            }

            // Draw notes
            let clip_start = clip.position_frames;
            for note in clip.notes.iter() {
                if note.pitch < min_note || note.pitch > max_note {
                    continue;
                }
                let row = (max_note - note.pitch) as usize;
                let rel_start_secs = note.start_frame as f64 / sr as f64;
                let dur_secs = note.duration as f64 / sr as f64;

                let y = origin.y + row as f32 * row_height - app.piano_roll_state.scroll_y as f32 + (row_height - note_bar_height) / 2.0;
                let x = grid_x + (rel_start_secs * pps) as f32 - app.piano_roll_state.scroll_x as f32;
                let w = (dur_secs * pps).max(4.0) as f32;

                if x + w < grid_x || x > response.rect.right() || y + note_bar_height < origin.y || y > bottom {
                    continue;
                }

                let note_rect = egui::Rect::from_min_size(pos2(x, y), vec2(w, note_bar_height));
                let is_playing = app.is_playing() && {
                    let cur_f = (app.position_seconds() * sr as f64) as u64;
                    cur_f >= clip_start + note.start_frame && cur_f < clip_start + note.start_frame + note.duration
                };

                let intensity = 0.4 + (note.velocity as f32 / 127.0) * 0.6;
                let base_color = if is_playing {
                    Color32::from_rgb(0x33, 0xcc, 0x33)
                } else {
                    Color32::from_rgb(
                        (0x8a as f32 * intensity) as u8,
                        (0x2b as f32 * intensity) as u8,
                        (0xe2 as f32 * intensity) as u8,
                    )
                };
                
                painter.rect_filled(note_rect, 2.0, base_color);
                painter.rect_stroke(note_rect, 2.0, egui::Stroke::new(1.0, Color32::from_rgb(0xaa, 0x55, 0xff)));

                // Note resize handle
                let resize_rect = egui::Rect::from_min_max(pos2(note_rect.right() - 6.0, note_rect.top()), note_rect.right_bottom());
                if response.hovered() && ui.rect_contains_pointer(resize_rect) {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                }
            }

            // Draw preview note during click-drag creation
            if let Some(PianoRollDragTarget::NoteCreate { pitch, start_frame, current_end_frame }) = &app.piano_roll_state.drag_target {
                if *pitch >= min_note && *pitch <= max_note {
                    let row = (max_note - *pitch) as usize;
                    let dur_frames = current_end_frame.saturating_sub(*start_frame);
                    let rel_start = *start_frame as f64 / sr as f64;
                    let dur_secs = dur_frames as f64 / sr as f64;
                    let y = origin.y + row as f32 * row_height - app.piano_roll_state.scroll_y as f32 + (row_height - note_bar_height) / 2.0;
                    let x = grid_x + (rel_start * pps) as f32 - app.piano_roll_state.scroll_x as f32;
                    let w = (dur_secs * pps).max(4.0) as f32;
                    if x + w > grid_x && x < response.rect.right() && y + note_bar_height > origin.y && y < bottom {
                        let preview_rect = egui::Rect::from_min_size(pos2(x, y), vec2(w, note_bar_height));
                        painter.rect_filled(preview_rect, 2.0, Color32::from_rgba_premultiplied(0x8a, 0x2b, 0xe2, 180));
                    }
                }
            }

            // Handle interactions
            if response.drag_started() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let rel_x = pos.x - grid_x + app.piano_roll_state.scroll_x as f32;
                    let rel_y = pos.y - origin.y + app.piano_roll_state.scroll_y as f32;
                    let row = (rel_y / row_height) as usize;
                    if row < num_rows {
                        let pitch = max_note - row as u8;
                        let rel_frame = (rel_x as f64 / pps * sr as f64) as u64;

                        let hit = clip.notes.iter().enumerate().find(|(_, n)| {
                            n.pitch == pitch && rel_frame >= n.start_frame && rel_frame < n.start_frame + n.duration
                        });

                        if let Some((idx, n)) = hit {
                            let rel_start_secs = n.start_frame as f64 / sr as f64;
                            let dur_secs = n.duration as f64 / sr as f64;
                            let note_x = grid_x + (rel_start_secs * pps) as f32 - app.piano_roll_state.scroll_x as f32;
                            let note_w = (dur_secs * pps) as f32;

                            if pos.x > note_x + note_w - 8.0 {
                                app.piano_roll_state.drag_target = Some(PianoRollDragTarget::NoteResize { note_idx: idx, original_duration: n.duration });
                            } else {
                                app.piano_roll_state.drag_target = Some(PianoRollDragTarget::NoteMove { note_idx: idx, original_note: n.clone() });
                            }
                        } else {
                            // Click-drag on empty space: start creating a new note
                            let snapped_start = app.timeline_state.snap_frames_to_grid(rel_frame, sr, bpm, &app.preferences, &app.project.markers);
                            app.piano_roll_state.drag_target = Some(PianoRollDragTarget::NoteCreate {
                                pitch,
                                start_frame: snapped_start,
                                current_end_frame: snapped_start,
                            });
                        }
                    }
                }
            }

            if response.dragged() {
                let delta_x = response.drag_delta().x as f64;
                let delta_y = response.drag_delta().y as f64;
                let frames_delta = (delta_x / pps * sr as f64).round() as i64;

                // For NoteCreate, recompute end_frame from current pointer position
                if matches!(app.piano_roll_state.drag_target, Some(PianoRollDragTarget::NoteCreate { .. })) {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let rel_x = pos.x - grid_x + app.piano_roll_state.scroll_x as f32;
                        let current_frame = (rel_x as f64 / pps * sr as f64).max(0.0) as u64;
                        if let Some(PianoRollDragTarget::NoteCreate { pitch, start_frame, .. }) = app.piano_roll_state.drag_target.take() {
                            app.piano_roll_state.drag_target = Some(PianoRollDragTarget::NoteCreate {
                                pitch,
                                start_frame,
                                current_end_frame: current_frame.max(start_frame + 1),
                            });
                        }
                    }
                } else if let Some(target) = app.piano_roll_state.drag_target.clone() {
                    match target {
                        PianoRollDragTarget::NoteMove { note_idx, original_note } => {
                            let pitch_delta = -(delta_y / row_height as f64).round() as i32;
                            let new_pitch = (original_note.pitch as i32 + pitch_delta).clamp(0, 127) as u8;
                            let new_start = (original_note.start_frame as i64 + frames_delta).max(0) as u64;

                            let mut new_note = original_note.clone();
                            new_note.pitch = new_pitch;
                            new_note.start_frame = app.timeline_state.snap_frames_to_grid(new_start, sr, bpm, &app.preferences, &app.project.markers);

                            app.update_midi_note(track_idx, clip_id, note_idx, new_note);
                        }
                        PianoRollDragTarget::NoteResize { note_idx, original_duration } => {
                            let new_dur = (original_duration as i64 + frames_delta).max(sr as i64 / 100) as u64;
                            let dur = app.timeline_state.snap_frames_to_grid(new_dur, sr, bpm, &app.preferences, &app.project.markers).max(1);
                            let mut n = clip.notes[note_idx].clone();
                            n.duration = dur;
                            app.update_midi_note(track_idx, clip_id, note_idx, n);
                        }
                        _ => {}
                    }
                }
            }

            if response.drag_stopped() {
                if let Some(PianoRollDragTarget::NoteCreate { pitch, start_frame, current_end_frame }) = app.piano_roll_state.drag_target.take() {
                    let dur = current_end_frame.saturating_sub(start_frame).max(1);
                    app.add_midi_note(track_idx, clip_id, MidiNote {
                        pitch,
                        velocity: app.preferences.piano_roll_default_velocity,
                        release_velocity: 64,
                        start_frame,
                        duration: dur,
                    });
                } else {
                    app.piano_roll_state.drag_target = None;
                }
            }

            // Note deletion (right-click / secondary click)
            if response.clicked_by(egui::PointerButton::Secondary) && !response.dragged() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let rel_x = pos.x - grid_x + app.piano_roll_state.scroll_x as f32;
                    let rel_y = pos.y - origin.y + app.piano_roll_state.scroll_y as f32;
                    let row = (rel_y / row_height) as usize;
                    if row < num_rows {
                        let pitch = max_note - row as u8;
                        let rel_frame = (rel_x as f64 / pps * sr as f64) as u64;
                        let hit_idx = clip.notes.iter().position(|n| {
                            n.pitch == pitch && rel_frame >= n.start_frame && rel_frame < n.start_frame + n.duration
                        });
                        if let Some(idx) = hit_idx {
                            app.remove_midi_note(track_idx, clip_id, idx);
                        }
                    }
                }
            }

            // Note creation (primary click on empty space)
            if response.clicked() && !response.dragged() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let rel_x = pos.x - grid_x + app.piano_roll_state.scroll_x as f32;
                    let rel_y = pos.y - origin.y + app.piano_roll_state.scroll_y as f32;
                    let row = (rel_y / row_height) as usize;
                    if row < num_rows {
                        let pitch = max_note - row as u8;
                        let rel_frame = (rel_x as f64 / pps * sr as f64) as u64;

                        let hit = clip.notes.iter().any(|n| {
                            n.pitch == pitch && rel_frame >= n.start_frame && rel_frame < n.start_frame + n.duration
                        });

                        if !hit {
                            let snapped_start = app.timeline_state.snap_frames_to_grid(rel_frame, sr, bpm, &app.preferences, &app.project.markers);
                            let frames_per_beat = (sr as f64 / bps).round() as u64;
                            let dur = (app.piano_roll_state.note_length * frames_per_beat as f64).round() as u64;
                            
                            app.add_midi_note(track_idx, clip_id, MidiNote {
                                pitch,
                                velocity: app.preferences.piano_roll_default_velocity,
                                release_velocity: 64,
                                start_frame: snapped_start,
                                duration: dur.max(1),
                            });
                        }
                    }
                }
            }

            // Separator at grid bottom when lane is active
            if has_lane {
                painter.line_segment(
                    [pos2(note_names_x, bottom), pos2(right, bottom)],
                    egui::Stroke::new(1.0, Color32::from_gray(80)),
                );
            }

            if has_lane {
                match app.piano_roll_state.controller_lane {
                    ControllerLane::Velocity | ControllerLane::ReleaseVelocity => {
                        let (lane_label, lane_color) = match app.piano_roll_state.controller_lane {
                            ControllerLane::Velocity => ("Vel", Color32::from_rgb(0x4c, 0xaf, 0x50)),
                            ControllerLane::ReleaseVelocity => ("Rel", Color32::from_rgb(0xe0, 0x8c, 0x30)),
                            _ => unreachable!(),
                        };

                        let pad = 3.0;
                        let (vel_response, vel_painter) = ui.allocate_painter(
                            vec2(available.x, VEL_LANE_HEIGHT),
                            egui::Sense::click_and_drag(),
                        );
                        let vel_rect = vel_response.rect;
                        let vel_content_left = vel_rect.left() + NOTE_NAME_WIDTH;
                        let vel_content_right = vel_rect.right();

                        vel_painter.rect_filled(vel_rect, 0.0, Color32::from_gray(18));
                        vel_painter.text(
                            pos2(vel_rect.left() + 4.0, vel_rect.top() + 2.0),
                            egui::Align2::LEFT_TOP,
                            lane_label,
                            egui::FontId::proportional(LANE_LABEL_FONT_SIZE),
                            Color32::from_gray(140),
                        );

                        let bar_top = vel_rect.top() + pad;
                        let bar_bottom = vel_rect.bottom() - pad;
                        let bar_range = (bar_bottom - bar_top).max(1.0);

                        for note in clip.notes.iter() {
                            let rel_start_secs = note.start_frame as f64 / sr as f64;
                            let dur_secs = note.duration as f64 / sr as f64;
                            let x = vel_content_left + (rel_start_secs * pps) as f32
                                - app.piano_roll_state.scroll_x as f32;
                            let w = (dur_secs * pps).max(4.0) as f32;
                            if x + w < vel_content_left || x > vel_content_right {
                                continue;
                            }
                            let raw = match app.piano_roll_state.controller_lane {
                                ControllerLane::Velocity => note.velocity,
                                ControllerLane::ReleaseVelocity => note.release_velocity,
                                _ => unreachable!(),
                            };
                            let bar_h = (raw as f32 / 127.0) * bar_range;
                            let bar_rect = egui::Rect::from_min_max(
                                pos2(x, bar_bottom - bar_h),
                                pos2((x + w).min(vel_content_right), bar_bottom),
                            );
                            vel_painter.rect_filled(bar_rect, 0.0, lane_color);
                            vel_painter.rect_stroke(bar_rect, 0.0, egui::Stroke::new(1.0, Color32::from_gray(60)));
                        }

                        let ref_y = bar_bottom - bar_range * 0.5;
                        vel_painter.line_segment(
                            [pos2(vel_content_left, ref_y), pos2(vel_content_right, ref_y)],
                            egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(120, 120, 120, 40)),
                        );

                        if vel_response.drag_started() || (vel_response.clicked() && !vel_response.dragged()) {
                            if let Some(pos) = vel_response.interact_pointer_pos() {
                                let rel_x = pos.x - vel_content_left + app.piano_roll_state.scroll_x as f32;
                                let click_frame = (rel_x as f64 / pps * sr as f64).max(0.0) as u64;
                                let hit = clip.notes.iter().position(|n| {
                                    click_frame >= n.start_frame && click_frame < n.start_frame + n.duration
                                });
                                if let Some(idx) = hit {
                                    let vel = ((bar_bottom - pos.y).clamp(0.0, bar_range) / bar_range * 127.0)
                                        .clamp(0.0, 127.0) as u8;
                                    let mut new_note = clip.notes[idx].clone();
                                    match app.piano_roll_state.controller_lane {
                                        ControllerLane::Velocity => new_note.velocity = vel,
                                        ControllerLane::ReleaseVelocity => new_note.release_velocity = vel,
                                        _ => {}
                                    }
                                    app.update_midi_note(track_idx, clip_id, idx, new_note);
                                    if vel_response.drag_started() {
                                        app.piano_roll_state.controller_drag_note = Some(idx);
                                    }
                                }
                            }
                        }

                        if vel_response.dragged() {
                            if let Some(note_idx) = app.piano_roll_state.controller_drag_note {
                                if let Some(pos) = vel_response.interact_pointer_pos() {
                                    let vel = ((bar_bottom - pos.y).clamp(0.0, bar_range) / bar_range * 127.0)
                                        .clamp(0.0, 127.0) as u8;
                                    if note_idx < clip.notes.len() {
                                        let mut new_note = clip.notes[note_idx].clone();
                                        match app.piano_roll_state.controller_lane {
                                            ControllerLane::Velocity => new_note.velocity = vel,
                                            ControllerLane::ReleaseVelocity => new_note.release_velocity = vel,
                                            _ => {}
                                        }
                                        app.update_midi_note(track_idx, clip_id, note_idx, new_note);
                                    }
                                }
                            }
                        }

                        if vel_response.drag_stopped() {
                            app.piano_roll_state.controller_drag_note = None;
                        }
                    }

                    ControllerLane::Cc(cc_n) => {
                        let cc_label = format!("CC {cc_n}");
                        let cc_color = Color32::from_rgb(0x42, 0xa5, 0xf5);
                        let scroll_x = app.piano_roll_state.scroll_x;

                        let (cc_response, cc_painter) = ui.allocate_painter(
                            vec2(available.x, VEL_LANE_HEIGHT),
                            egui::Sense::click_and_drag(),
                        );
                        let cc_rect = cc_response.rect;
                        let cc_content_left = cc_rect.left() + NOTE_NAME_WIDTH;
                        let cc_content_right = cc_rect.right();

                        cc_painter.rect_filled(cc_rect, 0.0, Color32::from_gray(18));
                        cc_painter.text(
                            pos2(cc_rect.left() + 4.0, cc_rect.top() + 2.0),
                            egui::Align2::LEFT_TOP,
                            cc_label,
                            egui::FontId::proportional(LANE_LABEL_FONT_SIZE),
                            Color32::from_gray(140),
                        );

                        let pad = 3.0;
                        let curve_top = cc_rect.top() + pad;
                        let curve_bottom = cc_rect.bottom() - pad;
                        let curve_range = (curve_bottom - curve_top).max(1.0);

                        let mut filtered: Vec<&crate::project::cc_event::CCEvent> = clip.cc_events.iter()
                            .filter(|e| e.cc_number == cc_n)
                            .collect();
                        filtered.sort_by_key(|e| e.time_frames);

                        let ref_y = curve_bottom - curve_range * 0.5;
                        cc_painter.line_segment(
                            [pos2(cc_content_left, ref_y), pos2(cc_content_right, ref_y)],
                            egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(120, 120, 120, 40)),
                        );

                        let mut prev_screen = None;
                        for event in &filtered {
                            let x = cc_content_left + (event.time_frames as f64 / sr as f64 * pps) as f32
                                - scroll_x as f32;
                            let y = curve_bottom - (event.value * curve_range);
                            let pt = pos2(x, y);

                            if let Some(prev) = prev_screen {
                                cc_painter.line_segment(
                                    [prev, pt],
                                    egui::Stroke::new(1.5, cc_color),
                                );
                            }
                            prev_screen = Some(pt);

                            cc_painter.circle_filled(pt, CC_CIRCLE_RADIUS, cc_color);
                            cc_painter.circle_stroke(pt, CC_CIRCLE_RADIUS, egui::Stroke::new(1.0, Color32::from_gray(200)));
                        }

                        // CC point editing
                        let hit_radius = CC_HIT_RADIUS;
                        let find_cc_point = |pos: egui::Pos2| -> Option<usize> {
                            filtered.iter().position(|e| {
                                let x = cc_content_left + (e.time_frames as f64 / sr as f64 * pps) as f32
                                    - scroll_x as f32;
                                let y = curve_bottom - (e.value * curve_range);
                                (pos.x - x).abs() < hit_radius && (pos.y - y).abs() < hit_radius
                            })
                        };

                        let find_cc_segment = |pos: egui::Pos2| -> Option<(usize, u64, f32)> {
                            for i in 0..filtered.len().saturating_sub(1) {
                                let a = filtered[i];
                                let b = filtered[i + 1];
                                let ax = cc_content_left + (a.time_frames as f64 / sr as f64 * pps) as f32
                                    - scroll_x as f32;
                                let bx = cc_content_left + (b.time_frames as f64 / sr as f64 * pps) as f32
                                    - scroll_x as f32;
                                if pos.x >= ax && pos.x <= bx {
                                    let t = if (bx - ax).abs() > 0.5 {
                                        ((pos.x - ax) / (bx - ax)) as f64
                                    } else {
                                        0.5
                                    };
                                    let time_frames = a.time_frames as f64 + (b.time_frames as f64 - a.time_frames as f64) * t;
                                    let value = a.value + (b.value - a.value) * t as f32;
                                    let seg_y = curve_bottom - (value * curve_range);
                                    if (pos.y - seg_y).abs() < 12.0 {
                                        return Some((i, time_frames.round() as u64, value.clamp(0.0, 1.0)));
                                    }
                                }
                            }
                            None
                        };

                        if cc_response.drag_started() || (cc_response.clicked() && !cc_response.dragged()) {
                            if let Some(pos) = cc_response.interact_pointer_pos() {
                                if let Some(pi) = find_cc_point(pos) {
                                    app.piano_roll_state.cc_drag = Some(crate::ui::piano_roll_state::CcDragState {
                                        event_idx: pi,
                                        original_event: filtered[pi].clone(),
                                    });
                                } else if let Some((_, time_frames, value)) = find_cc_segment(pos) {
                                    let event = crate::project::cc_event::CCEvent { cc_number: cc_n, time_frames, value };
                                    app.add_midi_cc_event(track_idx, clip_id, event);
                                }
                            }
                        }

                        if cc_response.dragged() {
                            if let Some(ref drag) = app.piano_roll_state.cc_drag.clone() {
                                if let Some(pos) = cc_response.interact_pointer_pos() {
                                    let rel_x = pos.x - cc_content_left + app.piano_roll_state.scroll_x as f32;
                                    let time_frames = (rel_x as f64 / pps * sr as f64).max(0.0) as u64;
                                    let value = ((curve_bottom - pos.y) / curve_range).clamp(0.0, 1.0);
                                    let new_event = crate::project::cc_event::CCEvent {
                                        cc_number: cc_n,
                                        time_frames,
                                        value,
                                    };
                                    app.update_midi_cc_event(track_idx, clip_id, &drag.original_event, new_event);
                                }
                            }
                        }

                        if cc_response.drag_stopped() {
                            if let Some(drag) = app.piano_roll_state.cc_drag.take() {
                                if let Some(pos) = cc_response.interact_pointer_pos() {
                                    let rel_x = pos.x - cc_content_left + app.piano_roll_state.scroll_x as f32;
                                    let time_frames = (rel_x as f64 / pps * sr as f64).max(0.0) as u64;
                                    let value = ((curve_bottom - pos.y) / curve_range).clamp(0.0, 1.0);
                                    let new_event = crate::project::cc_event::CCEvent {
                                        cc_number: cc_n,
                                        time_frames,
                                        value,
                                    };
                                    app.undo_service.push(crate::app::undo::UndoCommand::MoveCcEvent {
                                        track_index: track_idx,
                                        clip_id,
                                        event_index: drag.event_idx,
                                        old_event: drag.original_event,
                                        new_event,
                                    });
                                }
                            }
                        }

                        if cc_response.clicked_by(egui::PointerButton::Secondary) {
                            if let Some(pos) = cc_response.interact_pointer_pos() {
                                if let Some(pi) = find_cc_point(pos) {
                                    let del_event = &filtered[pi];
                                    app.remove_midi_cc_event(track_idx, clip_id, del_event, pi);
                                }
                            }
                        }
                    }
                    ControllerLane::None => {}
                }
            }
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        show = false;
    }

    if !show {
        app.show_piano_roll = false;
        app.editing_midi_clip_id = None;
    }
}
