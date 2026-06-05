use crate::audio::effects::dsp_effect::EffectType;
use crate::dsp::biquad;
use egui::{pos2, Color32, Pos2, Rect, Shape, Stroke, Vec2};

const GRAPH_HEIGHT: f32 = 120.0;
const GRAPH_MARGIN: f32 = 8.0;
const LABEL_FONT_SIZE: f32 = 8.0;
const NUM_SAMPLE_POINTS: usize = 200;
const FREQ_LABEL_WIDTH: f32 = 30.0;

pub(super) struct FxData {
    pub(super) type_name: String,
    pub(super) etype: EffectType,
    pub(super) bypass: bool,
    pub(super) params: Vec<ParamData>,
}

pub(super) struct ParamData {
    pub(super) id: u32,
    pub(super) name: String,
    pub(super) value: f32,
    pub(super) min: f32,
    pub(super) max: f32,
}

pub(super) fn draw(ui: &mut egui::Ui, fx: &FxData, sample_rate: u32) {
    let (id, rect) = ui.allocate_space(Vec2::new(ui.available_width(), GRAPH_HEIGHT));
    let _ = ui.interact(rect, id, egui::Sense::hover());
    let painter = ui.painter();
    let bg = Rect::from_min_size(rect.min, Vec2::new(rect.width(), rect.height()));
    painter.rect_filled(bg, 2.0, Color32::from_rgb(0x15, 0x15, 0x15));
    painter.rect_stroke(bg, 2.0, Stroke::new(1.0, Color32::from_rgb(0x33, 0x33, 0x33)));

    let margin = GRAPH_MARGIN;
    let graph = Rect::from_min_max(
        pos2(rect.left() + margin, rect.top() + margin),
        pos2(rect.right() - margin, rect.bottom() - margin),
    );

    let zero_y = graph.center().y;
    painter.line_segment(
        [pos2(graph.left(), zero_y), pos2(graph.right(), zero_y)],
        Stroke::new(1.0, Color32::from_rgb(0x3a, 0x3a, 0x3a)),
    );

    fn label_font() -> egui::FontId { egui::FontId::proportional(LABEL_FONT_SIZE) }
    painter.text(pos2(graph.left(), graph.bottom() + 6.0), egui::Align2::LEFT_TOP, "20Hz", label_font(), Color32::from_gray(100));
    painter.text(pos2(graph.right() - FREQ_LABEL_WIDTH, graph.bottom() + 6.0), egui::Align2::LEFT_TOP, "20kHz", label_font(), Color32::from_gray(100));
    painter.text(pos2(graph.left() - 2.0, graph.top()), egui::Align2::RIGHT_TOP, "+15dB", label_font(), Color32::from_gray(100));
    painter.text(pos2(graph.left() - 2.0, graph.bottom() - 8.0), egui::Align2::RIGHT_BOTTOM, "-15dB", label_font(), Color32::from_gray(100));

    let freq_min = 20.0;
    let freq_max = 20000.0;
    let db_min = -15.0;
    let db_max = 15.0;
    let num_points = NUM_SAMPLE_POINTS;

    let btypes = [
        biquad::BiquadType::LowShelf,
        biquad::BiquadType::Peaking,
        biquad::BiquadType::Peaking,
        biquad::BiquadType::HighShelf,
    ];

    let pts: Vec<Pos2> = (0..=num_points).map(|i| {
        let t = i as f32 / num_points as f32;
        let ratio: f32 = freq_max / freq_min;
        let freq_hz = freq_min * ratio.powf(t);
        let sr = sample_rate as f32;
        let mut total_db = 0.0;

        for (band, btype) in btypes.iter().enumerate() {
            let fi = band * 3;
            let freq = fx.params.get(fi).map(|p| p.value).unwrap_or(80.0);
            let gain = fx.params.get(fi + 1).map(|p| p.value).unwrap_or(0.0);
            let q = fx.params.get(fi + 2).map(|p| p.value).unwrap_or(0.7).max(0.1);
            let c = biquad::compute_coeffs(btype, freq, gain, q, sr);
            total_db += biquad::frequency_response(&c, freq_hz, sr);
        }

        let db_clamped = total_db.clamp(db_min, db_max);
        let x = graph.left() + t * graph.width();
        let y = graph.bottom() - ((db_clamped - db_min) / (db_max - db_min)) * graph.height();
        pos2(x, y)
    }).collect();

    if pts.len() > 1 {
        let shape = Shape::line(pts, Stroke::new(1.5, Color32::from_rgb(0x42, 0xa5, 0xf5)));
        painter.add(shape);
    }

    for band in 0..4 {
        let fi = band * 3;
        let freq = fx.params.get(fi).map(|p| p.value).unwrap_or(80.0);
        let t = (freq / freq_min).log10() / (freq_max / freq_min).log10();
        let x = graph.left() + t * graph.width();
        painter.line_segment(
            [pos2(x, graph.top()), pos2(x, graph.bottom())],
            Stroke::new(1.0, Color32::from_rgba_premultiplied(0x88, 0x88, 0x44, 0x40)),
        );
    }
}
