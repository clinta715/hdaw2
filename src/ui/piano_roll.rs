use crate::app::HdawApp;
use crate::project::clip::ClipKind;
use crate::project::midi_note::MidiNote;
use egui::{pos2, vec2, Color32, Vec2};


const NOTE_NAME_WIDTH: f32 = 48.0;
const NOTE_BAR_HEIGHT_RATIO: f32 = 0.7; // Ratio of row height

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
    let pps = app.timeline_state.pixels_per_second;

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
    let row_height = app.preferences.piano_roll_row_height;
    let note_bar_height = row_height * NOTE_BAR_HEIGHT_RATIO;
    let num_rows = (max_note.saturating_sub(min_note) as usize).max(1) + 1;

    let mut show = app.show_piano_roll;
    let _window_response = egui::Window::new(format!("Piano Roll - {}", clip.name))
        .id("piano_roll".into())
        .open(&mut show)
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(700.0, 450.0))
        .show(ctx, |ui| {
            let available = ui.available_size();
            let grid_area = available.x - NOTE_NAME_WIDTH;
            if grid_area <= 0.0 {
                return;
            }

            let (response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());
            let origin = response.rect.left_top();

            let note_names_x = origin.x;
            let grid_x = origin.x + NOTE_NAME_WIDTH;
            let bottom = response.rect.bottom();

            for i in 0..num_rows {
                let note = max_note - i as u8;
                let y = origin.y + i as f32 * row_height;
                let row_end = (y + row_height).min(bottom);
                if row_end <= origin.y {
                    continue;
                }

                let row_bg = if is_white_key(note) {
                    Color32::from_rgb(0x2c, 0x2c, 0x2c)
                } else {
                    Color32::from_rgb(0x22, 0x22, 0x22)
                };

                painter.rect_filled(
                    egui::Rect::from_min_size(pos2(grid_x, y), vec2(grid_area, row_height)),
                    0.0,
                    row_bg,
                );

                painter.rect_filled(
                    egui::Rect::from_min_size(pos2(note_names_x, y), vec2(NOTE_NAME_WIDTH, row_height)),
                    0.0,
                    if note % 12 == 0 {
                        Color32::from_rgb(0x3a, 0x3a, 0x3a)
                    } else {
                        row_bg
                    },
                );

                if is_white_key(note) || note % 12 == 0 {
                    painter.text(
                        pos2(note_names_x + 4.0, y + row_height / 2.0),
                        egui::Align2::LEFT_CENTER,
                        note_to_name(note),
                        egui::FontId::proportional(9.0),
                        Color32::from_gray(140),
                    );
                }
            }

            let step = app.timeline_state.beat_step(bpm, app.preferences.grid_division);
            let bps = bpm / 60.0;
            let pixels_per_beat = pps / bps;
            
            let total_secs = clip.length_frames as f64 / sr as f64;
            let total_beats = (total_secs * bps).ceil() as usize;

            let base_alpha = (app.preferences.grid_opacity * 255.0) as u8;

            for beat_idx in 0..=(total_beats as f32 / step as f32) as usize {
                let beat = beat_idx as f64 * step;
                let x = grid_x + (beat * pixels_per_beat) as f32;
                if x > response.rect.right() {
                    break;
                }
                
                let is_bar = (beat / 4.0).fract().abs() < 0.001;
                let is_beat = (beat / 1.0).fract().abs() < 0.001;
                
                let alpha_mult = if is_bar { 1.0 } else if is_beat { 0.5 } else { 0.2 };
                let alpha = (base_alpha as f32 * alpha_mult) as u8;
                
                painter.line_segment(
                    [pos2(x, origin.y), pos2(x, bottom)],
                    egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(100, 100, 120, alpha)),
                );
            }

            let current_pos = app.position_seconds();
            let play_x = grid_x + (current_pos * pps) as f32;
            if play_x >= grid_x && play_x <= response.rect.right() {
                painter.line_segment(
                    [pos2(play_x, origin.y), pos2(play_x, bottom)],
                    egui::Stroke::new(2.0, Color32::from_rgb(0xff, 0xcc, 0x33)),
                );
            }

            let clip_start = clip.position_frames;
            let notes = &clip.notes;
            for note in notes {
                if note.pitch < min_note || note.pitch > max_note {
                    continue;
                }
                let row = (max_note - note.pitch) as usize;
                let abs_start = clip_start + note.start_frame;
                let abs_end = abs_start + note.duration;
                let rel_start_secs = (abs_start as f64 - clip_start as f64) / sr as f64;
                let dur_secs = note.duration as f64 / sr as f64;

                let y = origin.y + row as f32 * row_height + (row_height - note_bar_height) / 2.0;
                let x = grid_x + (rel_start_secs * pps) as f32;
                let w = (dur_secs * pps).max(3.0) as f32;

                let is_playing = current_pos * (sr as f64) >= abs_start as f64
                    && current_pos * (sr as f64) < abs_end as f64;

                let note_rect = egui::Rect::from_min_size(pos2(x, y), vec2(w, note_bar_height));
                let intensity = 0.3 + (note.velocity as f32 / 127.0) * 0.7;
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
            }

            if response.clicked() || response.double_clicked() {
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    if pos.x >= grid_x && pos.y >= origin.y && pos.y < bottom {
                        let rel_x = pos.x - grid_x;
                        let rel_y = pos.y - origin.y;
                        let row = (rel_y / row_height) as usize;
                        if row < num_rows {
                            let pitch = max_note - row as u8;
                            let secs = rel_x as f64 / pps;
                            let rel_frame = (secs * sr as f64).round() as u64;

                            let hit_idx = notes.iter().position(|n| {
                                n.pitch == pitch && rel_frame >= n.start_frame
                                    && rel_frame < n.start_frame + n.duration
                            });

                            if ui.input(|i| i.pointer.any_click() && i.pointer.secondary_down()) {
                                if let Some(idx) = hit_idx {
                                    app.remove_midi_note(track_idx, clip_id, idx);
                                    ui.ctx().request_repaint();
                                }
                            } else if response.clicked() && !ui.input(|i| i.pointer.secondary_down()) {
                                if hit_idx.is_none() {
                                    // Snap note start
                                    let snapped_rel_frame = app.timeline_state.snap_frames_to_grid(rel_frame, sr, bpm, &app.preferences, &app.project.markers);
                                    
                                    let frames_per_beat = (sr as f64 / bps).round() as u64;
                                    let dur = (step * frames_per_beat as f64).round() as u64;
                                    let note = MidiNote {
                                        pitch,
                                        velocity: app.preferences.piano_roll_default_velocity,
                                        start_frame: snapped_rel_frame,
                                        duration: dur.max(1),
                                    };
                                    app.add_midi_note(track_idx, clip_id, note);
                                    ui.ctx().request_repaint();
                                }
                            }
                        }
                    }
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
