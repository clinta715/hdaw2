use crate::app::HdawApp;
use crate::ui::timeline::{TimelineState, PLAYHEAD_WIDTH};
use egui::{pos2, Color32, Rect, Stroke};

pub fn draw(painter: &egui::Painter, rect: Rect, state: &TimelineState, app: &HdawApp, header_width: f32) {
    let pos_secs = app.position_seconds();
    let x = rect.left() + header_width
        + (pos_secs * state.pixels_per_second - state.scroll_x) as f32;
    if x >= rect.left() + header_width && x <= rect.right() {
        painter.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            Stroke::new(PLAYHEAD_WIDTH, Color32::from_rgb(0xff, 0x44, 0x44)),
        );
    }
}
