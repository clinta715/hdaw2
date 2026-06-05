use egui::{pos2, vec2, Color32, Pos2, Rect, Stroke};
use std::sync::atomic::Ordering;

const BODY_FONT_SIZE: f32 = 11.0;
const SMALL_FONT_SIZE: f32 = 9.0;
const INFO_FONT_SIZE: f32 = 8.5;

#[derive(Clone, Default)]
pub struct TrackFxInfo {
    pub instrument_name: Option<String>,
    pub fx_names: Vec<String>,
}

pub fn draw(
    painter: &egui::Painter,
    rect: &Rect,
    track: &crate::app::TrackUiState,
    is_selected: bool,
    is_expanded: bool,
    fx_info: &TrackFxInfo,
) {
    let bg = if is_selected {
        Color32::from_rgb(0x2a, 0x35, 0x45)
    } else {
        Color32::from_rgb(0x22, 0x22, 0x22)
    };
    painter.rect_filled(*rect, 0.0, bg);

    let border = if is_selected {
        Color32::from_rgb(0x64, 0xb5, 0xf6)
    } else {
        Color32::from_rgb(0x33, 0x33, 0x33)
    };
    painter.rect_stroke(*rect, 0.0, Stroke::new(1.0, border));

    let color_strip = Rect::from_min_size(rect.min, vec2(6.0, rect.height()));
    let strip_color = Color32::from_rgb(track.color[0], track.color[1], track.color[2]);
    painter.rect_filled(color_strip, 0.0, strip_color);

    let text_x = rect.left() + 10.0;
    let body_font = egui::FontId::proportional(BODY_FONT_SIZE);
    let small_font = egui::FontId::proportional(SMALL_FONT_SIZE);

    // Track Name
    painter.text(
        pos2(text_x, rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        &track.name,
        body_font.clone(),
        Color32::from_gray(220),
    );

    // Volume Bar
    let vol = f32::from_bits(track.volume.load(Ordering::Acquire));
    let vol_bar_rect = volume_rect(rect);
    painter.rect_filled(vol_bar_rect, 1.0, Color32::from_rgb(0x1a, 0x2a, 0x1a));
    let vol_fill = vol_bar_rect.width() * vol.min(1.0);
    if vol_fill > 0.0 {
        painter.rect_filled(
            Rect::from_min_size(vol_bar_rect.min, vec2(vol_fill, vol_bar_rect.height())),
            1.0,
            Color32::from_rgb(0x4c, 0xaf, 0x50),
        );
    }
    
    let vol_db = if vol <= 0.0 { "-inf".to_string() } else { format!("{:.1}dB", 20.0 * vol.log10()) };
    painter.text(
        vol_bar_rect.center(),
        egui::Align2::CENTER_CENTER,
        vol_db,
        small_font.clone(),
        Color32::from_gray(200),
    );

    // Pan Bar
    let pan = f32::from_bits(track.pan.load(Ordering::Acquire)); // 0.0 = Left, 0.5 = Center, 1.0 = Right
    let pan_bar_rect = pan_rect(rect);
    painter.rect_filled(pan_bar_rect, 1.0, Color32::from_rgb(0x1a, 0x1a, 0x2a));
    
    let center_x = pan_bar_rect.center().x;
    let pan_handle_x = pan_bar_rect.left() + (pan * pan_bar_rect.width());
    painter.line_segment(
        [pos2(center_x, pan_bar_rect.top()), pos2(center_x, pan_bar_rect.bottom())],
        Stroke::new(1.0, Color32::from_gray(100)),
    );
    
    let pan_color = Color32::from_rgb(0x64, 0xb5, 0xf6);
    let (p1, p2) = if pan < 0.5 {
        (pos2(pan_handle_x, pan_bar_rect.top() + 1.0), pos2(center_x, pan_bar_rect.bottom() - 1.0))
    } else {
        (pos2(center_x, pan_bar_rect.top() + 1.0), pos2(pan_handle_x, pan_bar_rect.bottom() - 1.0))
    };
    painter.rect_filled(Rect::from_min_max(p1, p2), 0.0, pan_color);

    let pan_label = if (pan - 0.5).abs() < 0.02 { "C".to_string() }
                   else if pan < 0.5 { format!("L{:.0}", (0.5 - pan) * 200.0) }
                   else { format!("R{:.0}", (pan - 0.5) * 200.0) };
    painter.text(
        pan_bar_rect.center(),
        egui::Align2::CENTER_CENTER,
        pan_label,
        small_font.clone(),
        Color32::from_gray(200),
    );

    // Collapse arrow for group tracks
    if track.is_group {
        let arrow_rect = collapse_btn_rect(rect);
        let arrow = if track.collapsed { "▸" } else { "▾" };
        painter.text(arrow_rect.center(), egui::Align2::CENTER_CENTER, arrow, body_font.clone(), Color32::from_gray(160));
    }

    let expand_btn = expand_btn_rect(rect);
    let expand_bg = if is_expanded { Color32::from_rgb(0x42, 0xa5, 0xf5) } else { Color32::from_gray(60) };
    painter.rect_filled(expand_btn, 1.0, expand_bg);
    painter.text(expand_btn.center(), egui::Align2::CENTER_CENTER, "⇕", small_font.clone(), Color32::from_gray(220));

    // Buttons
    let mute = track.mute.load(Ordering::Acquire);
    let mute_btn = mute_btn_rect(rect);
    painter.rect_filled(mute_btn, 1.0, if mute { Color32::from_rgb(0xcc, 0x33, 0x33) } else { Color32::from_gray(60) });
    painter.text(mute_btn.center(), egui::Align2::CENTER_CENTER, "M", small_font.clone(), Color32::from_gray(220));

    let solo = track.solo.load(Ordering::Acquire);
    let solo_btn = solo_btn_rect(rect);
    painter.rect_filled(solo_btn, 1.0, if solo { Color32::from_rgb(0xcc, 0xcc, 0x33) } else { Color32::from_gray(60) });
    painter.text(solo_btn.center(), egui::Align2::CENTER_CENTER, "S", small_font.clone(), Color32::from_gray(220));

    if !track.is_group && !track.is_return {
        let armed = track.armed.load(Ordering::Acquire);
        let arm_btn = arm_btn_rect(rect);
        painter.rect_filled(arm_btn, 1.0, if armed { Color32::from_rgb(0xcc, 0x33, 0x33) } else { Color32::from_gray(60) });
        painter.text(arm_btn.center(), egui::Align2::CENTER_CENTER, "R", small_font.clone(), Color32::from_gray(220));
    }

    let info_y = rect.top() + 68.0;
    let info_font = egui::FontId::proportional(INFO_FONT_SIZE);
    let info_max_x = rect.right() - 22.0;

    if let Some(ref inst) = fx_info.instrument_name {
        let inst_text = format!("\u{266b} {}", inst);
        let truncated = truncate_text(&inst_text, &info_font, text_x, info_max_x);
        painter.text(
            pos2(text_x, info_y),
            egui::Align2::LEFT_TOP,
            truncated,
            info_font.clone(),
            Color32::from_rgb(0x4d, 0xd0, 0xe1),
        );
    }

    if !fx_info.fx_names.is_empty() {
        let fx_y = if fx_info.instrument_name.is_some() { info_y + 10.0 } else { info_y };
        let fx_text = fx_info.fx_names.join(", ");
        let truncated = truncate_text(&fx_text, &info_font, text_x, info_max_x);
        painter.text(
            pos2(text_x, fx_y),
            egui::Align2::LEFT_TOP,
            truncated,
            info_font.clone(),
            Color32::from_gray(160),
        );
    }

    // Meters (Vertical on the right)
    let meter_x = rect.right() - 18.0;
    let meter_y = rect.top() + 4.0;
    let meter_h = rect.height() - 8.0;
    let meter_w = 12.0;
    let meter_rect = Rect::from_min_size(pos2(meter_x, meter_y), vec2(meter_w, meter_h));
    painter.rect_filled(meter_rect, 0.0, Color32::from_rgb(0x12, 0x12, 0x12));

    let peak_l = f32::from_bits(track.peak_left.load(Ordering::Acquire));
    let peak_r = f32::from_bits(track.peak_right.load(Ordering::Acquire));
    
    for (ch, peak) in [peak_l, peak_r].iter().enumerate() {
        let x = meter_x + ch as f32 * 6.0;
        let fill_h = meter_h * peak.min(1.0);
        if fill_h > 0.0 {
            let color = if *peak > 0.9 { Color32::from_rgb(0xcc, 0x33, 0x33) } else { Color32::from_rgb(0x4c, 0xaf, 0x50) };
            painter.rect_filled(
                Rect::from_min_max(pos2(x, meter_rect.bottom() - fill_h), pos2(x + 5.0, meter_rect.bottom())),
                0.0,
                color,
            );
        }
    }
    painter.rect_stroke(meter_rect, 0.0, Stroke::new(1.0, Color32::from_gray(60)));
}

pub fn volume_rect(rect: &Rect) -> Rect {
    Rect::from_min_size(pos2(rect.left() + 10.0, rect.top() + 20.0), vec2(rect.width() - 32.0, 12.0))
}

pub fn pan_rect(rect: &Rect) -> Rect {
    Rect::from_min_size(pos2(rect.left() + 10.0, rect.top() + 34.0), vec2(rect.width() - 32.0, 12.0))
}

pub fn mute_btn_rect(rect: &Rect) -> Rect {
    Rect::from_min_size(pos2(rect.left() + 10.0, rect.top() + 50.0), vec2(24.0, 16.0))
}

pub fn solo_btn_rect(rect: &Rect) -> Rect {
    Rect::from_min_size(pos2(rect.left() + 38.0, rect.top() + 50.0), vec2(24.0, 16.0))
}

pub fn arm_btn_rect(rect: &Rect) -> Rect {
    Rect::from_min_size(pos2(rect.left() + 66.0, rect.top() + 50.0), vec2(24.0, 16.0))
}

pub fn expand_btn_rect(rect: &Rect) -> Rect {
    Rect::from_min_size(pos2(rect.right() - 52.0, rect.top() + 50.0), vec2(18.0, 16.0))
}

pub fn collapse_btn_rect(rect: &Rect) -> Rect {
    Rect::from_min_size(pos2(rect.left() + 8.0, rect.top() + 2.0), vec2(16.0, 16.0))
}

pub fn hit_test(rect: &Rect, pos: Pos2, is_group: bool, _is_return: bool) -> HeaderAction {
    if !rect.contains(pos) { return HeaderAction::None; }
    if is_group && collapse_btn_rect(rect).contains(pos) { return HeaderAction::ToggleCollapse; }
    if expand_btn_rect(rect).contains(pos) { return HeaderAction::ToggleExpand; }
    if mute_btn_rect(rect).contains(pos) { return HeaderAction::ToggleMute; }
    if solo_btn_rect(rect).contains(pos) { return HeaderAction::ToggleSolo; }
    if arm_btn_rect(rect).contains(pos) { return HeaderAction::ToggleArm; }
    if volume_rect(rect).contains(pos) { return HeaderAction::Volume; }
    if pan_rect(rect).contains(pos) { return HeaderAction::Pan; }
    HeaderAction::Select
}

pub enum HeaderAction {
    None,
    Select,
    ToggleMute,
    ToggleSolo,
    ToggleArm,
    ToggleCollapse,
    ToggleExpand,
    Volume,
    Pan,
}

fn truncate_text(text: &str, _font: &egui::FontId, min_x: f32, max_x: f32) -> String {
    let max_width = max_x - min_x;
    if max_width <= 0.0 { return String::new(); }

    let mut result = String::new();
    let mut current_width = 0.0;
    let char_width = 5.5;

    for ch in text.chars() {
        let w = match ch {
            '\u{266b}' => 7.0,
            'A'..='Z' | '0'..='9' => 6.0,
            _ => char_width,
        };
        if current_width + w > max_width {
            break;
        }
        result.push(ch);
        current_width += w;
    }

    if result.len() < text.len() {
        result.push('\u{2026}');
    }

    result
}
