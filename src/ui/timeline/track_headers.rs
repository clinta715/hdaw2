use egui::{pos2, vec2, Color32, Pos2, Rect, Stroke};
use std::sync::atomic::Ordering;

pub fn draw(
    painter: &egui::Painter,
    rect: &Rect,
    track: &crate::app::TrackUiState,
    is_selected: bool,
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
    let body_font = egui::FontId::proportional(12.0);
    let small_font = egui::FontId::proportional(10.0);

    painter.text(
        pos2(text_x, rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        &track.name,
        body_font,
        Color32::from_gray(200),
    );

    let vol = f32::from_bits(track.volume.load(Ordering::Acquire));
    let vol_db = if vol <= 0.0 {
        "-inf".to_string()
    } else {
        format!("{:.1} dB", 20.0 * vol.log10())
    };
    painter.text(
        pos2(text_x, rect.top() + 22.0),
        egui::Align2::LEFT_TOP,
        &vol_db,
        small_font.clone(),
        Color32::from_gray(150),
    );

    let vol_bar_rect = Rect::from_min_size(
        pos2(text_x, rect.top() + 36.0),
        vec2((rect.width() - 14.0).max(1.0), 6.0),
    );
    let vol_fill = vol_bar_rect.width() * vol.min(1.0);
    painter.rect_filled(vol_bar_rect, 1.0, Color32::from_rgb(0x33, 0x44, 0x33));
    painter.rect_filled(
        Rect::from_min_size(vol_bar_rect.min, vec2(vol_fill, vol_bar_rect.height())),
        1.0,
        Color32::from_rgb(0x4c, 0xaf, 0x50),
    );

    let mute = track.mute.load(Ordering::Acquire);
    let mute_color = if mute {
        Color32::from_rgb(0xcc, 0x33, 0x33)
    } else {
        Color32::from_rgb(0x55, 0x55, 0x55)
    };
    let mute_btn = btn_rect(rect, 0);
    painter.rect_filled(mute_btn, 2.0, mute_color);
    painter.text(
        mute_btn.center(),
        egui::Align2::CENTER_CENTER,
        "M",
        small_font.clone(),
        Color32::from_gray(200),
    );

    let solo = track.solo.load(Ordering::Acquire);
    let solo_color = if solo {
        Color32::from_rgb(0xcc, 0xcc, 0x33)
    } else {
        Color32::from_rgb(0x55, 0x55, 0x55)
    };
    let solo_btn = btn_rect(rect, 1);
    painter.rect_filled(solo_btn, 2.0, solo_color);
    painter.text(
        solo_btn.center(),
        egui::Align2::CENTER_CENTER,
        "S",
        small_font.clone(),
        Color32::from_gray(200),
    );

    let peak_y = rect.top() + 64.0;
    let peak_l = f32::from_bits(track.peak_left.load(Ordering::Acquire));
    let peak_r = f32::from_bits(track.peak_right.load(Ordering::Acquire));
    let peak_h = 10.0;

    for (ch, peak) in [(0u16, peak_l), (1, peak_r)].iter() {
        let x = rect.right() - 16.0 - *ch as f32 * 8.0;
        let bar_rect = Rect::from_min_size(pos2(x, peak_y), vec2(6.0, peak_h));
        painter.rect_filled(bar_rect, 1.0, Color32::from_rgb(0x1a, 0x33, 0x1a));
        let fill_h = peak_h * peak.min(1.0);
        if fill_h > 0.0 {
            let fill_color = if *peak > 0.9 {
                Color32::from_rgb(0xcc, 0x33, 0x33)
            } else {
                Color32::from_rgb(0x4c, 0xaf, 0x50)
            };
            painter.rect_filled(
                Rect::from_min_size(pos2(x, peak_y + peak_h - fill_h), vec2(6.0, fill_h)),
                1.0,
                fill_color,
            );
        }
    }
}

fn btn_rect(rect: &Rect, index: usize) -> Rect {
    let text_x = rect.left() + 10.0;
    let x = text_x + index as f32 * 24.0;
    Rect::from_min_size(pos2(x, rect.top() + 46.0), vec2(20.0, 16.0))
}

pub fn mute_btn_rect(rect: &Rect) -> Rect {
    btn_rect(rect, 0)
}

pub fn solo_btn_rect(rect: &Rect) -> Rect {
    btn_rect(rect, 1)
}

pub fn hit_test(rect: &Rect, pos: Pos2) -> HeaderAction {
    if !rect.contains(pos) {
        return HeaderAction::None;
    }
    if mute_btn_rect(rect).contains(pos) {
        return HeaderAction::ToggleMute;
    }
    if solo_btn_rect(rect).contains(pos) {
        return HeaderAction::ToggleSolo;
    }
    HeaderAction::Select
}

pub enum HeaderAction {
    None,
    Select,
    ToggleMute,
    ToggleSolo,
}
