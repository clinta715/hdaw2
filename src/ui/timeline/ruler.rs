use crate::project::marker::Marker;
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
) {
    let ruler_rect = Rect::from_min_size(rect.min, vec2(rect.width(), RULER_HEIGHT));
    let ruler_bg = Color32::from_rgb(0x2a, 0x2a, 0x2a);
    painter.rect_filled(ruler_rect, 0.0, ruler_bg);

    let pps = state.pixels_per_second;
    let step = state.grid_step_secs();

    let start_time = (state.scroll_x / pps).max(0.0);
    let end_time = (state.scroll_x + rect.width() as f64) / pps;

    // Draw loop region overlay
    if loop_enabled {
        if let (Some(in_f), Some(out_f)) = (loop_in, loop_out) {
            let sr = sample_rate as f64;
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
                // Loop handle triangles
                let handle_size = 8.0;
                // In handle
                painter.line_segment(
                    [pos2(in_x, rect.top()), pos2(in_x + handle_size, rect.top() + handle_size)],
                    Stroke::new(2.0, Color32::from_rgb(0x44, 0x88, 0xcc)),
                );
                painter.line_segment(
                    [pos2(in_x, rect.top() + RULER_HEIGHT), pos2(in_x + handle_size, rect.top() + RULER_HEIGHT - handle_size)],
                    Stroke::new(2.0, Color32::from_rgb(0x44, 0x88, 0xcc)),
                );
                // Out handle
                painter.line_segment(
                    [pos2(out_x, rect.top()), pos2(out_x - handle_size, rect.top() + handle_size)],
                    Stroke::new(2.0, Color32::from_rgb(0x44, 0x88, 0xcc)),
                );
                painter.line_segment(
                    [pos2(out_x, rect.top() + RULER_HEIGHT), pos2(out_x - handle_size, rect.top() + RULER_HEIGHT - handle_size)],
                    Stroke::new(2.0, Color32::from_rgb(0x44, 0x88, 0xcc)),
                );
            }
        }
    }

    // Draw ticks
    let first_tick = (start_time / step).ceil() * step;
    let mut t = first_tick;
    let font_id = egui::FontId::proportional(10.0);
    while t <= end_time {
        let x = rect.left() + (t * pps - state.scroll_x) as f32;
        if x < rect.left() || x > rect.right() {
            t += step;
            continue;
        }
        let is_major = ((t / step).round() as i64) % 2 == 0;
        let tick_h = if is_major { 12.0 } else { 6.0 };
        let tick_y = rect.top() + RULER_HEIGHT - tick_h;
        let color = Color32::from_gray(120);
        painter.line_segment(
            [pos2(x, tick_y), pos2(x, rect.top() + RULER_HEIGHT)],
            Stroke::new(1.0, color),
        );
        if is_major {
            let mins = (t / 60.0) as u32;
            let secs = (t % 60.0) as u32;
            let label = format!("{:02}:{:02}", mins, secs);
            painter.text(
                pos2(x + 2.0, rect.top() + 2.0),
                egui::Align2::LEFT_TOP,
                label,
                font_id.clone(),
                Color32::from_gray(180),
            );
        }
        t += step;
    }

    // Draw markers
    let sr = sample_rate as f64;
    for marker in markers {
        let pos_secs = marker.position_frames as f64 / sr;
        let x = rect.left() + (pos_secs * pps - state.scroll_x) as f32;
        if x < rect.left() || x > rect.right() { continue; }
        // Diamond flag
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
        // Label
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