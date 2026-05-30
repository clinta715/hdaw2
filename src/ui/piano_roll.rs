use crate::app::HdawApp;
use crate::project::clip::ClipKind;
use crate::project::midi_note::MidiNote;
use egui::{pos2, vec2, Color32, Vec2};


const NOTE_NAME_WIDTH: f32 = 48.0;
const ROW_HEIGHT: f32 = 14.0;
const MIN_NOTE: u8 = 24;
const MAX_NOTE: u8 = 96;
const NOTE_BAR_HEIGHT: f32 = 10.0;

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

    let num_rows = (MAX_NOTE - MIN_NOTE) as usize;

    egui::Window::new(format!("Piano Roll - {}", clip.name))
        .id("piano_roll".into())
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
                let note = MAX_NOTE - i as u8;
                let y = origin.y + i as f32 * ROW_HEIGHT;
                let row_end = (y + ROW_HEIGHT).min(bottom);
                if row_end <= origin.y {
                    continue;
                }

                let row_bg = if is_white_key(note) {
                    Color32::from_rgb(0x2c, 0x2c, 0x2c)
                } else {
                    Color32::from_rgb(0x22, 0x22, 0x22)
                };

                painter.rect_filled(
                    egui::Rect::from_min_size(pos2(grid_x, y), vec2(grid_area, ROW_HEIGHT)),
                    0.0,
                    row_bg,
                );

                painter.rect_filled(
                    egui::Rect::from_min_size(pos2(note_names_x, y), vec2(NOTE_NAME_WIDTH, ROW_HEIGHT)),
                    0.0,
                    if note % 12 == 0 {
                        Color32::from_rgb(0x3a, 0x3a, 0x3a)
                    } else {
                        row_bg
                    },
                );

                if is_white_key(note) || note % 12 == 0 {
                    painter.text(
                        pos2(note_names_x + 4.0, y + ROW_HEIGHT / 2.0),
                        egui::Align2::LEFT_CENTER,
                        note_to_name(note),
                        egui::FontId::proportional(9.0),
                        Color32::from_gray(140),
                    );
                }
            }

            let secs_per_beat = 60.0 / bpm as f64;
            let total_secs = clip.length_frames as f64 / sr as f64;
            let total_beats = (total_secs / secs_per_beat).ceil() as usize;

            for beat in 0..=total_beats {
                let secs = beat as f64 * secs_per_beat;
                let x = grid_x + (secs * pps) as f32;
                if x > response.rect.right() {
                    break;
                }
                let grid_color = if beat % 4 == 0 {
                    Color32::from_rgba_premultiplied(100, 100, 120, 80)
                } else {
                    Color32::from_rgba_premultiplied(80, 80, 90, 50)
                };
                painter.line_segment(
                    [pos2(x, origin.y), pos2(x, bottom)],
                    egui::Stroke::new(1.0, grid_color),
                );
            }

            let current_pos = app.position_seconds();
            let play_x = grid_x + (current_pos * pps) as f32;
            painter.line_segment(
                [pos2(play_x, origin.y), pos2(play_x, bottom)],
                egui::Stroke::new(2.0, Color32::from_rgb(0xff, 0xcc, 0x33)),
            );

            let clip_start = clip.position_frames;
            let notes = &clip.notes;
            for note in notes {
                let row = (MAX_NOTE - note.pitch) as usize;
                if row >= num_rows {
                    continue;
                }
                let abs_start = clip_start + note.start_frame;
                let abs_end = abs_start + note.duration;
                let rel_start_secs = (abs_start as f64 - clip_start as f64) / sr as f64;
                let dur_secs = note.duration as f64 / sr as f64;

                let y = origin.y + row as f32 * ROW_HEIGHT + (ROW_HEIGHT - NOTE_BAR_HEIGHT) / 2.0;
                let x = grid_x + (rel_start_secs * pps) as f32;
                let w = (dur_secs * pps).max(3.0) as f32;

                let is_playing = current_pos * (sr as f64) >= abs_start as f64
                    && current_pos * (sr as f64) < abs_end as f64;

                let note_rect = egui::Rect::from_min_size(pos2(x, y), vec2(w, NOTE_BAR_HEIGHT));
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
                        let row = (rel_y / ROW_HEIGHT) as usize;
                        if row < num_rows {
                            let pitch = MAX_NOTE - row as u8;
                            let secs = rel_x as f64 / pps;
                            let frame = (secs * sr as f64).round() as u64;

                            let hit_idx = notes.iter().position(|n| {
                                n.pitch == pitch && frame >= n.start_frame
                                    && frame < n.start_frame + n.duration
                            });

                            if ui.input(|i| i.pointer.any_click() && i.pointer.secondary_down()) {
                                if let Some(idx) = hit_idx {
                                    app.remove_midi_note(track_idx, clip_id, idx);
                                    ui.ctx().request_repaint();
                                }
                            } else if response.clicked() && !ui.input(|i| i.pointer.secondary_down()) {
                                if hit_idx.is_none() {
                                    let dur = (secs_per_beat * sr as f64).round() as u64;
                                    let note = MidiNote {
                                        pitch,
                                        velocity: 100,
                                        start_frame: frame,
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

            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                app.show_piano_roll = false;
                app.editing_midi_clip_id = None;
            }
        });
}
