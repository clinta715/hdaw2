use crate::project::marker::Marker;
use crate::project::tempo_event::{TempoEvent, TimeSigEvent, frames_to_beats, beats_to_frames};
use crate::ui::timeline::{TimelineState, RULER_HEIGHT};
use egui::{pos2, vec2, Color32, Rect, Stroke};

pub fn draw(
    painter: &egui::Painter,
    rect: Rect,
    state: &TimelineState,
    markers: &[Marker],
    loop_in: Option<u64>,
    loop_out: Option<u64>,
    loop_enabled: bool,
    sample_rate: u32,
    bpm: f64,
    tempo_events: &[TempoEvent],
    _time_sig_events: &[TimeSigEvent],
    prefs: &crate::ui::preferences::PreferencesState,
) {
    let ruler_rect = Rect::from_min_size(rect.min, vec2(rect.width(), RULER_HEIGHT));
    let ruler_bg = Color32::from_rgb(0x2a, 0x2a, 0x2a);
    painter.rect_filled(ruler_rect, 0.0, ruler_bg);

    let pps = state.pixels_per_second;
    let sr = sample_rate as f64;

    // Use tempo_events if available, otherwise single BPM
    let events = if tempo_events.is_empty() {
        &[]
    } else {
        tempo_events
    };

    let use_tempo_track = !events.is_empty();

    // Visible range in frames
    let start_secs = state.scroll_x / pps;
    let end_secs = (state.scroll_x + rect.width() as f64) / pps;
    let start_frame = (start_secs * sr).max(0.0) as u64;
    let end_frame = (end_secs * sr) as u64;

    // Tempo-aware beat range
    let start_beat = if use_tempo_track {
        frames_to_beats(start_frame, events, sample_rate)
    } else {
        let bps = bpm / 60.0;
        start_secs * bps
    };

    let end_beat = if use_tempo_track {
        frames_to_beats(end_frame, events, sample_rate)
    } else {
        let bps = bpm / 60.0;
        end_secs * bps
    };

    // Loop region
    if loop_enabled {
        if let (Some(in_f), Some(out_f)) = (loop_in, loop_out) {
            let in_secs = in_f as f64 / sr;
            let out_secs = out_f as f64 / sr;
            let in_x = rect.left() + (in_secs * pps - state.scroll_x) as f32;
            let out_x = rect.left() + (out_secs * pps - state.scroll_x) as f32;
            if out_x > rect.left() && in_x < rect.right() {
                let l = in_x.max(rect.left());
                let r = out_x.min(rect.right());
                let loop_rect = Rect::from_min_max(pos2(l, rect.top()), pos2(r, rect.top() + RULER_HEIGHT));
                if r > l {
                    painter.rect_filled(loop_rect, 0.0, Color32::from_rgba_premultiplied(0x44, 0x88, 0xcc, 40));
                }
                let handle_size = 8.0;
                painter.line_segment([pos2(in_x, rect.top()), pos2(in_x + handle_size, rect.top() + handle_size)], Stroke::new(2.0, Color32::from_rgb(0x44, 0x88, 0xcc)));
                painter.line_segment([pos2(in_x, rect.top() + RULER_HEIGHT), pos2(in_x + handle_size, rect.top() + RULER_HEIGHT - handle_size)], Stroke::new(2.0, Color32::from_rgb(0x44, 0x88, 0xcc)));
                painter.line_segment([pos2(out_x, rect.top()), pos2(out_x - handle_size, rect.top() + handle_size)], Stroke::new(2.0, Color32::from_rgb(0x44, 0x88, 0xcc)));
                painter.line_segment([pos2(out_x, rect.top() + RULER_HEIGHT), pos2(out_x - handle_size, rect.top() + RULER_HEIGHT - handle_size)], Stroke::new(2.0, Color32::from_rgb(0x44, 0x88, 0xcc)));
            }
        }
    }

    // Draw musical ticks
    let step = state.beat_step(if use_tempo_track { tempo_at_event(events, start_frame) } else { bpm }, prefs.grid_division);
    let first_tick = (start_beat / step).ceil() * step;
    let font_id = egui::FontId::proportional(10.0);
    let mut beat = first_tick;
    while beat <= end_beat {
        // Convert beat to pixel x
        let x = if use_tempo_track {
            let f = beats_to_frames(beat, events, sample_rate);
            let secs = f as f64 / sr;
            rect.left() + (secs * pps - state.scroll_x) as f32
        } else {
            let secs = beat * 60.0 / bpm;
            rect.left() + (secs * pps - state.scroll_x) as f32
        };
        if x < rect.left() || x > rect.right() {
            beat += step;
            continue;
        }

        // Determine time sig for this beat
        let num = if use_tempo_track {
            let f = beats_to_frames(beat, events, sample_rate);
            crate::project::tempo_event::time_sig_at(&[], f).0 as f64
        } else {
            4.0
        };

        let is_bar = (beat / num).fract().abs() < 0.001;
        let is_beat_tick = (beat / 1.0).fract().abs() < 0.001;

        let tick_h = if is_bar { 14.0 } else if is_beat_tick { 8.0 } else { 4.0 };
        let tick_y = rect.top() + RULER_HEIGHT - tick_h;
        let color = if is_bar { Color32::from_gray(180) } else { Color32::from_gray(100) };

        painter.line_segment(
            [pos2(x, tick_y), pos2(x, rect.top() + RULER_HEIGHT)],
            Stroke::new(1.0, color),
        );

        if is_bar || (is_beat_tick && step >= 1.0) {
            let bar = (beat / num).floor() as u32 + 1;
            let beat_in_bar = (beat % num).floor() as u32 + 1;
            let label = if is_bar { format!("{}", bar) } else { format!("{}.{}", bar, beat_in_bar) };
            painter.text(
                pos2(x + 2.0, rect.top() + 2.0),
                egui::Align2::LEFT_TOP,
                label,
                font_id.clone(),
                if is_bar { Color32::from_gray(220) } else { Color32::from_gray(150) },
            );
        }
        beat += step;
    }

    // Draw tempo change indicators
    if use_tempo_track {
        for event in events {
            let secs = event.position_frames as f64 / sr;
            let x = rect.left() + (secs * pps - state.scroll_x) as f32;
            if x < rect.left() || x > rect.right() {
                continue;
            }
            // Small colored marker at top
            painter.line_segment(
                [pos2(x, rect.top()), pos2(x, rect.top() + 4.0)],
                Stroke::new(2.0, Color32::from_rgb(0xcc, 0x88, 0x44)),
            );
        }
    }

    // Draw markers
    for marker in markers {
        let pos_secs = marker.position_frames as f64 / sr;
        let x = rect.left() + (pos_secs * pps - state.scroll_x) as f32;
        if x < rect.left() || x > rect.right() { continue; }
        let mid_y = rect.top() + RULER_HEIGHT / 2.0;
        let size = 4.0;
        painter.add(egui::Shape::convex_polygon(
            vec![
                pos2(x, mid_y - size),
                pos2(x + size, mid_y),
                pos2(x, mid_y + size),
                pos2(x - size, mid_y),
            ],
            Color32::from_rgb(marker.color[0], marker.color[1], marker.color[2]),
            Stroke::new(0.0, Color32::TRANSPARENT),
        ));
        if x + 4.0 < rect.right() {
            painter.text(
                pos2(x + 4.0, rect.top() + 2.0),
                egui::Align2::LEFT_TOP,
                &marker.name,
                egui::FontId::proportional(9.0),
                Color32::from_rgb(marker.color[0], marker.color[1], marker.color[2]),
            );
        }
    }
}

fn tempo_at_event(events: &[TempoEvent], frame: u64) -> f64 {
    crate::project::tempo_event::tempo_at(events, frame)
}
