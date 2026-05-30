use crate::project::automation::{AutomationLane, PARAM_PAN, PARAM_VOLUME};
use crate::ui::timeline::TimelineState;
use egui::{pos2, Color32, Pos2, Rect, Shape, Stroke};

const POINT_RADIUS: f32 = 4.0;
const HIT_RADIUS: f32 = 6.0;

pub fn draw(
    painter: &egui::Painter,
    lane_rect: &Rect,
    lanes: &[AutomationLane],
    timeline: &TimelineState,
    sr: u32,
) {
    for lane in lanes {
        let color = match lane.param_id {
            PARAM_VOLUME => Color32::from_rgb(0x4c, 0xaf, 0x50),
            PARAM_PAN => Color32::from_rgb(0x42, 0xa5, 0xf5),
            _ => Color32::from_rgb(0xaa, 0xaa, 0xaa),
        };
        draw_lane(painter, lane_rect, lane, timeline, sr, color);
    }
}

fn draw_lane(
    painter: &egui::Painter,
    lane_rect: &Rect,
    lane: &AutomationLane,
    timeline: &TimelineState,
    sr: u32,
    color: Color32,
) {
    if lane.points.is_empty() {
        return;
    }

    let pps = timeline.pixels_per_second;
    let scroll_x = timeline.scroll_x;
    let sr = sr as f64;

    let pts: Vec<Pos2> = lane
        .points
        .iter()
        .filter_map(|p| {
            let x = lane_rect.left()
                + (p.time_frames as f64 / sr * pps - scroll_x) as f32;
            if x < lane_rect.left() - 20.0 || x > lane_rect.right() + 20.0 {
                return None;
            }
            let y = param_y(lane.param_id, p.value, lane_rect);
            Some(pos2(x, y))
        })
        .collect();

    if pts.len() < 2 {
        if let Some(pt) = pts.first() {
            painter.circle_filled(*pt, POINT_RADIUS, color);
        }
        return;
    }

    let shape = Shape::line(pts.clone(), Stroke::new(1.5, color));
    painter.add(shape);

    for pt in &pts {
        painter.circle_filled(*pt, POINT_RADIUS, color);
        painter.circle_stroke(
            *pt,
            POINT_RADIUS,
            Stroke::new(1.0, Color32::from_gray(200)),
        );
    }
}

fn param_y(param_id: u32, value: f32, lane_rect: &Rect) -> f32 {
    let top = lane_rect.top() + 4.0;
    let bottom = lane_rect.bottom() - 4.0;
    let height = (bottom - top).max(1.0);
    match param_id {
        PARAM_VOLUME => {
            let v = value.clamp(0.0, 1.0);
            bottom - v * height
        }
        PARAM_PAN => {
            let v = value.clamp(-1.0, 1.0);
            let center = (top + bottom) * 0.5;
            center - v * (height * 0.5)
        }
        _ => bottom - value * height,
    }
}

pub fn param_value_from_y(param_id: u32, y: f32, lane_rect: &Rect) -> f32 {
    let top = lane_rect.top() + 4.0;
    let bottom = lane_rect.bottom() - 4.0;
    let height = (bottom - top).max(1.0);
    match param_id {
        PARAM_VOLUME => {
            let t = ((bottom - y) / height).clamp(0.0, 1.0);
            t
        }
        PARAM_PAN => {
            let center = (top + bottom) * 0.5;
            let t = ((center - y) / (height * 0.5)).clamp(-1.0, 1.0);
            t
        }
        _ => ((bottom - y) / height).clamp(0.0, 1.0),
    }
}

pub fn find_point(
    lanes: &[AutomationLane],
    mouse_pos: Pos2,
    lane_rect: &Rect,
    timeline: &TimelineState,
    sr: u32,
) -> Option<(usize, usize)> {
    let pps = timeline.pixels_per_second;
    let scroll_x = timeline.scroll_x;
    let sr = sr as f64;

    for (li, lane) in lanes.iter().enumerate() {
        for (pi, point) in lane.points.iter().enumerate() {
            let x = lane_rect.left()
                + (point.time_frames as f64 / sr * pps - scroll_x) as f32;
            let y = param_y(lane.param_id, point.value, lane_rect);
            let dist = ((mouse_pos.x - x).powi(2) + (mouse_pos.y - y).powi(2)).sqrt();
            if dist < HIT_RADIUS {
                return Some((li, pi));
            }
        }
    }
    None
}

pub fn find_segment(
    lanes: &[AutomationLane],
    mouse_pos: Pos2,
    lane_rect: &Rect,
    timeline: &TimelineState,
    sr: u32,
) -> Option<(usize, f64, f32)> {
    let pps = timeline.pixels_per_second;
    let scroll_x = timeline.scroll_x;
    let sr = sr as f64;

    for (li, lane) in lanes.iter().enumerate() {
        if lane.points.len() < 2 {
            continue;
        }
        for pair in lane.points.windows(2) {
            let a = &pair[0];
            let b = &pair[1];
            let ax = lane_rect.left()
                + (a.time_frames as f64 / sr * pps - scroll_x) as f32;
            let bx = lane_rect.left()
                + (b.time_frames as f64 / sr * pps - scroll_x) as f32;
            if mouse_pos.x >= ax && mouse_pos.x <= bx {
                let ay = param_y(lane.param_id, a.value, lane_rect);
                let by = param_y(lane.param_id, b.value, lane_rect);
                let t = ((mouse_pos.x - ax) / (bx - ax).max(1.0)) as f64;
                let line_y = ay + (by - ay) * t as f32;
                let dist = (mouse_pos.y - line_y).abs();
                if dist < 10.0 {
                    return Some((li, t, param_value_from_y(lane.param_id, mouse_pos.y, lane_rect)));
                }
            }
        }
    }
    None
}

pub fn add_point_to_lane(
    lane: &mut AutomationLane,
    time_frames: u64,
    value: f32,
) -> Option<crate::project::automation::AutomationPoint> {
    let point = crate::project::automation::AutomationPoint { time_frames, value };
    lane.add_point(time_frames, value);
    lane.dirty = true;
    Some(point)
}

pub fn remove_point(lane: &mut AutomationLane, point_idx: usize) {
    if point_idx < lane.points.len() {
        lane.points.remove(point_idx);
        lane.dirty = true;
    }
}
