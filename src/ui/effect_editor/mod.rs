mod eq_graph;

use crate::app::HdawApp;
use crate::audio::effects::create_effect;
use crate::audio::effects::dsp_effect::{EffectInstance, EffectType};
use egui::{Color32, Context, SidePanel, Slider, Vec2};

use eq_graph::{FxData, ParamData};

pub struct EffectEditorState {
    pub selected_track: Option<usize>,
    pub selected_effect: Option<usize>,
    pub show_editor: bool,
    pub show_add_menu: bool,
}

impl Default for EffectEditorState {
    fn default() -> Self {
        Self {
            selected_track: None,
            selected_effect: None,
            show_editor: false,
            show_add_menu: false,
        }
    }
}

fn read_fx_chain(app: &HdawApp, track_idx: usize) -> (Vec<FxData>, String) {
    let empty = String::new();
    let name = app.track_ui.get(track_idx).map(|t| t.name.clone()).unwrap_or(empty);
    let Ok(tracks) = app.engine.tracks.lock() else { return (Vec::new(), name) };
    let Some(track) = tracks.get(track_idx) else { return (Vec::new(), name) };
    let data = track.fx_chain.iter().map(|inst| FxData {
        type_name: format!("{:?}", inst.effect_type),
        etype: inst.effect_type,
        bypass: inst.is_bypassed(),
        params: inst.effect.parameter_info().iter().map(|p| ParamData {
            id: p.id, name: p.name.clone(),
            value: inst.effect.parameter_value(p.id),
            min: p.min_value, max: p.max_value,
        }).collect(),
    }).collect();
    (data, name)
}

fn write_bypass(app: &mut HdawApp, track_idx: usize, effect_idx: usize, bypass: bool) {
    if let Ok(mut ts) = app.engine.tracks.lock() {
        if let Some(t) = ts.get_mut(track_idx) {
            t.set_effect_bypass(effect_idx, bypass);
        }
    }
}

fn write_param(app: &mut HdawApp, track_idx: usize, effect_idx: usize, param_id: u32, value: f32) {
    if let Ok(ts) = app.engine.tracks.lock() {
        if let Some(t) = ts.get(track_idx) {
            if let Some(inst) = t.fx_chain.get(effect_idx) {
                inst.effect.set_parameter(param_id, value);
            }
        }
    }
}

fn remove_effect(app: &mut HdawApp, track_idx: usize, effect_idx: usize) {
    let serialized = if let Ok(mut ts) = app.engine.tracks.lock() {
        let serialized = ts.get(track_idx).and_then(|t| {
            t.fx_chain.get(effect_idx).map(|inst| {
                let pv: Vec<f32> = inst.effect.parameter_info().iter()
                    .map(|p| inst.effect.parameter_value(p.id)).collect();
                crate::project::track::SerializedEffect {
                    name: inst.name.clone(),
                    effect_type: inst.effect_type,
                    bypass: inst.is_bypassed(),
                    param_values: pv,
                }
            })
        });
        if let Some(t) = ts.get_mut(track_idx) {
            t.remove_effect(effect_idx);
        }
        serialized
    } else {
        None
    };
    if let Some(s) = serialized {
        app.undo_state.push(crate::app::undo::UndoCommand::RemoveEffect {
            track_index: track_idx,
            effect_index: effect_idx,
            serialized: s,
        });
    }
}

fn add_effect(app: &mut HdawApp, track_idx: usize, name: &str, etype: EffectType) {
    let instance = EffectInstance::new(name.to_string(), etype, create_effect(etype));
    let effect_index;
    let pv;
    if let Ok(mut ts) = app.engine.tracks.lock() {
        if let Some(t) = ts.get_mut(track_idx) {
            effect_index = t.fx_chain.len();
            t.add_effect(instance);
            pv = t.fx_chain.last().map(|inst| {
                inst.effect.parameter_info().iter()
                    .map(|p| inst.effect.parameter_value(p.id)).collect()
            }).unwrap_or_default();
        } else { return; }
    } else { return; }
    app.undo_state.push(crate::app::undo::UndoCommand::AddEffect {
        track_index: track_idx,
        effect_index,
        serialized: crate::project::track::SerializedEffect {
            name: name.to_string(),
            effect_type: etype,
            bypass: false,
            param_values: pv,
        },
    });
}

const EFFECT_TYPES: [(&str, EffectType); 5] = [
    ("Gain",       EffectType::Gain),
    ("EQ",         EffectType::Equalizer),
    ("Compressor", EffectType::Compressor),
    ("Reverb",     EffectType::Reverb),
    ("Delay",      EffectType::Delay),
];

pub fn render(ctx: &Context, app: &mut HdawApp) {
    if !app.effect_editor_state.show_editor {
        return;
    }

    let track_idx = match app.effect_editor_state.selected_track.or(app.selected_track) {
        Some(t) if t < app.track_ui.len() => t,
        _ => {
            SidePanel::right("effect_editor")
                .resizable(true)
                .default_width(280.0)
                .min_width(200.0)
                .show(ctx, |ui| {
                    ui.heading("FX Editor");
                    ui.separator();
                    ui.colored_label(Color32::from_gray(120), "No track selected.\nClick a track header to begin.");
                });
            return;
        }
    };

    if app.effect_editor_state.show_add_menu {
        app.effect_editor_state.show_add_menu = false;
        egui::Window::new("Select Effect")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
            .show(ctx, |ui| {
                for &(name, etype) in &EFFECT_TYPES {
                    if ui.button(name).clicked() {
                        add_effect(app, track_idx, name, etype);
                        ui.close_menu();
                    }
                }
            });
        return;
    }

    let (fx_data, track_name) = read_fx_chain(app, track_idx);
    let effect_count = fx_data.len();
    let mut selected = app.effect_editor_state.selected_effect;
    let mut dirty = false;

    SidePanel::right("effect_editor")
        .resizable(true)
        .default_width(280.0)
        .min_width(200.0)
        .show(ctx, |ui| {
            ui.heading("FX Editor");
            ui.label(format!("Track: {track_name}"));
            ui.separator();

            ui.horizontal(|ui| {
                for (ei, fx) in fx_data.iter().enumerate() {
                    let is_selected = selected == Some(ei);
                    let cc = if fx.bypass { Color32::from_gray(60) }
                    else if is_selected { Color32::from_rgb(0x3a, 0x6a, 0xaa) }
                    else { Color32::from_rgb(0x3a, 0x4a, 0x3a) };
                    if ui.add(egui::Button::new(&fx.type_name).fill(cc).min_size(Vec2::new(60.0, 24.0))).clicked() {
                        selected = Some(ei);
                    }
                }
            });

            if ui.button("+ Add Effect").clicked() {
                app.effect_editor_state.show_add_menu = true;
                dirty = true;
                return;
            }

            let ei = match selected { Some(e) if e < effect_count => e, _ => {
                if effect_count == 0 {
                    ui.separator();
                    ui.colored_label(Color32::from_gray(120), "No effects on this track.\nClick + Add Effect to start.");
                }
                return;
            }};
            ui.separator();

            if let Some(fx) = fx_data.get(ei) {
                ui.horizontal(|ui| {
                    ui.label(&fx.type_name);
                    let mut bp = fx.bypass;
                    if ui.checkbox(&mut bp, "Bypass").changed() {
                        write_bypass(app, track_idx, ei, bp);
                        dirty = true;
                        return;
                    }
                });

                if ui.button("Remove Effect").clicked() {
                    remove_effect(app, track_idx, ei);
                    selected = None;
                    dirty = true;
                    return;
                }

                if matches!(fx.etype, EffectType::Equalizer) {
                    ui.separator();
                    ui.label("Frequency Response");
                    eq_graph::draw(ui, fx, app.engine.transport.sample_rate());
                }

                ui.separator();

                for p in &fx.params {
                    let mut v = p.value;
                    if ui.add(Slider::new(&mut v, p.min..=p.max).text(&p.name)).changed() {
                        write_param(app, track_idx, ei, p.id, v);
                        dirty = true;
                        return;
                    }
                }
            }
        });

    app.effect_editor_state.selected_effect = selected;
    app.effect_editor_state.selected_track = Some(track_idx);
    if dirty {
        ctx.request_repaint();
    }
}
