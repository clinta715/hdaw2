mod eq_graph;

use crate::app::HdawApp;
use crate::audio::effects::create_effect;
use crate::audio::effects::dsp_effect::{EffectInstance, EffectType};
use crate::project::automation::AutomationLane;
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
        etype: inst.effect_type.clone(),
        bypass: inst.is_bypassed(),
        params: inst.parameter_info().iter().map(|p| ParamData {
            id: p.id, name: p.name.clone(),
            value: inst.parameter_value(p.id),
            min: p.min_value, max: p.max_value,
        }).collect(),
    }).collect();
    (data, name)
}

fn engine_fx_to_serialized(inst: &EffectInstance) -> crate::project::track::SerializedEffect {
    let pv: Vec<f32> = inst.parameter_info().iter()
        .map(|p| inst.parameter_value(p.id)).collect();
    crate::project::track::SerializedEffect {
        name: inst.name.clone(),
        effect_type: inst.effect_type.clone(),
        bypass: inst.is_bypassed(),
        param_values: pv,
    }
}

fn write_bypass(app: &mut HdawApp, track_idx: usize, effect_idx: usize, bypass: bool) {
    if let Ok(mut ts) = app.engine.tracks.lock() {
        if let Some(t) = ts.get_mut(track_idx) {
            t.set_effect_bypass(effect_idx, bypass);
        }
    }
    if let Some(track) = app.project.tracks.get_mut(track_idx) {
        if let Some(fx) = track.fx_chain.get_mut(effect_idx) {
            fx.bypass = bypass;
        }
    }
}

fn write_param(app: &mut HdawApp, track_idx: usize, effect_idx: usize, param_id: u32, value: f32) {
    if let Ok(mut ts) = app.engine.tracks.lock() {
        if let Some(t) = ts.get_mut(track_idx) {
            if let Some(inst) = t.fx_chain.get_mut(effect_idx) {
                inst.set_parameter(param_id, value);
            }
        }
    }
    if let Some(track) = app.project.tracks.get_mut(track_idx) {
        if let Some(fx) = track.fx_chain.get_mut(effect_idx) {
            if let Some(pv) = fx.param_values.get_mut(param_id as usize - 1) {
                *pv = value;
            }
        }
    }
}

fn remove_effect(app: &mut HdawApp, track_idx: usize, effect_idx: usize) {
    let result = if let Ok(mut ts) = app.engine.tracks.lock() {
        let effect_id = ts.get(track_idx).and_then(|t| t.fx_chain.get(effect_idx)).map(|inst| inst.id);
        let serialized = ts.get(track_idx).and_then(|t| {
            t.fx_chain.get(effect_idx).map(|inst| {
                let pv: Vec<f32> = inst.parameter_info().iter()
                    .map(|p| inst.parameter_value(p.id)).collect();
                crate::project::track::SerializedEffect {
                    name: inst.name.clone(),
                    effect_type: inst.effect_type.clone(),
                    bypass: inst.is_bypassed(),
                    param_values: pv,
                }
            })
        });
        if let Some(t) = ts.get_mut(track_idx) {
            t.remove_effect(effect_idx);
        }
        // Collect and remove automation lanes for this effect
        let removed_lanes: Vec<AutomationLane> = if let Some(eid) = effect_id {
            ts.get_mut(track_idx).map(|t| {
                let mut lanes: Vec<AutomationLane> = Vec::new();
                t.automation_lanes.retain(|l| {
                    if l.effect_instance_id == Some(eid) {
                        lanes.push(l.clone());
                        false
                    } else {
                        true
                    }
                });
                lanes
            }).unwrap_or_default()
        } else {
            Vec::new()
        };
        (serialized, removed_lanes, effect_id)
    } else {
        return;
    };
    let (serialized, removed_lanes, effect_id) = result;
    if let Some(s) = serialized {
        if let Some(track) = app.project.tracks.get_mut(track_idx) {
            if effect_idx < track.fx_chain.len() {
                track.fx_chain.remove(effect_idx);
            }
            if let Some(eid) = effect_id {
                track.automation_lanes.retain(|l| l.effect_instance_id != Some(eid));
            }
        }
        app.undo_state.push(crate::app::undo::UndoCommand::RemoveEffect {
            track_index: track_idx,
            effect_index: effect_idx,
            serialized: s,
            removed_lanes,
        });
    }
}

fn add_builtin_effect(app: &mut HdawApp, track_idx: usize, name: &str, etype: EffectType) {
    let instance = EffectInstance::new_builtin(name.to_string(), etype.clone(), create_effect(etype.clone()));
    let effect_index;
    let serialized;
    if let Ok(mut ts) = app.engine.tracks.lock() {
        if let Some(t) = ts.get_mut(track_idx) {
            effect_index = t.fx_chain.len();
            t.add_effect(instance);
            serialized = t.fx_chain.last().map(|inst| engine_fx_to_serialized(inst)).unwrap();
        } else { return; }
    } else { return; }
    if let Some(track) = app.project.tracks.get_mut(track_idx) {
        let idx = effect_index.min(track.fx_chain.len());
        track.fx_chain.insert(idx, serialized.clone());
    }
    app.undo_state.push(crate::app::undo::UndoCommand::AddEffect {
        track_index: track_idx,
        effect_index,
        serialized,
    });
}

fn add_clap_effect(app: &mut HdawApp, track_idx: usize, desc: &crate::audio::clap_scanner::PluginDescriptor) {
    let sr = app.engine.transport.sample_rate();
    let adapter = match crate::audio::clap_effect::ClapEffectAdapter::new_instance(&desc.id, &desc.path, sr) {
        Ok(a) => a,
        Err(e) => {
            app.error_message = Some(format!("Failed to load CLAP plugin {}: {}", desc.name, e));
            return;
        }
    };
    let etype = EffectType::Clap {
        plugin_id: desc.id.clone(),
        path: desc.path.to_string_lossy().into_owned(),
    };
    let instance = EffectInstance::new_clap(desc.name.clone(), etype.clone(), adapter);
    let effect_index;
    let serialized;
    if let Ok(mut ts) = app.engine.tracks.lock() {
        if let Some(t) = ts.get_mut(track_idx) {
            effect_index = t.fx_chain.len();
            t.add_effect(instance);
            serialized = t.fx_chain.last().map(|inst| engine_fx_to_serialized(inst)).unwrap();
        } else { return; }
    } else { return; }
    if let Some(track) = app.project.tracks.get_mut(track_idx) {
        let idx = effect_index.min(track.fx_chain.len());
        track.fx_chain.insert(idx, serialized.clone());
    }
    app.undo_state.push(crate::app::undo::UndoCommand::AddEffect {
        track_index: track_idx,
        effect_index,
        serialized,
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
                .default_width(app.preferences.effect_panel_width)
                .min_width(200.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("FX Editor");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("\u{2715}").clicked() {
                                app.effect_editor_state.show_editor = false;
                            }
                        });
                    });
                    ui.separator();
                    ui.colored_label(Color32::from_gray(120), "No track selected.\nClick a track header to begin.");
                });
            return;
        }
    };

    if app.effect_editor_state.show_add_menu {
        egui::Window::new("Select Effect")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("Built-in");
                for (name, etype) in &EFFECT_TYPES {
                    if ui.button(*name).clicked() {
                        add_builtin_effect(app, track_idx, name, etype.clone());
                        app.effect_editor_state.show_add_menu = false;
                    }
                }
                if !app.plugin_registry.is_empty() {
                    ui.separator();
                    ui.label("CLAP Plugins");
                    let descriptors = app.plugin_registry.clone();
                    for desc in &descriptors {
                        let label = if desc.is_instrument {
                            format!("{} [instrument]", desc.name)
                        } else {
                            desc.name.clone()
                        };
                        if ui.button(label).clicked() {
                            add_clap_effect(app, track_idx, desc);
                            app.effect_editor_state.show_add_menu = false;
                        }
                    }
                }
                ui.separator();
                if ui.button("Cancel").clicked() {
                    app.effect_editor_state.show_add_menu = false;
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
        .default_width(app.preferences.effect_panel_width)
        .min_width(200.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("FX Editor");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("\u{2715}").clicked() { // Cross symbol
                        app.effect_editor_state.show_editor = false;
                    }
                });
            });
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

                let inst_id = if let Ok(ts) = app.engine.tracks.lock() {
                    ts.get(track_idx).and_then(|t| t.fx_chain.get(ei)).map(|inst| inst.id)
                } else {
                    None
                };

                for p in &fx.params {
                    ui.horizontal(|ui| {
                        let mut v = p.value;
                        if ui.add(Slider::new(&mut v, p.min..=p.max).text(&p.name)).changed() {
                            write_param(app, track_idx, ei, p.id, v);
                            dirty = true;
                            return;
                        }
                        // "A" automate toggle
                        if let Some(inst_id) = inst_id {
                            let has_lane = if let Ok(ts) = app.engine.tracks.lock() {
                                ts.get(track_idx).map(|t| {
                                    t.automation_lanes.iter().any(|l| l.effect_instance_id == Some(inst_id) && l.param_id == p.id)
                                }).unwrap_or(false)
                            } else {
                                false
                            };
                            let label = if has_lane { "A" } else { "a" };
                            let btn_color = if has_lane { Color32::from_rgb(0x6a, 0xaa, 0x3a) } else { Color32::from_gray(80) };
                            if ui.add(egui::Button::new(label).fill(btn_color).min_size(egui::vec2(20.0, 18.0))).clicked() {
                                if has_lane {
                                    // Remove lane from both models
                                    if let Ok(mut ts) = app.engine.tracks.lock() {
                                        if let Some(t) = ts.get_mut(track_idx) {
                                            t.automation_lanes.retain(|l| !(l.effect_instance_id == Some(inst_id) && l.param_id == p.id));
                                        }
                                    }
                                    if let Some(track) = app.project.tracks.get_mut(track_idx) {
                                        track.automation_lanes.retain(|l| !(l.effect_instance_id == Some(inst_id) && l.param_id == p.id));
                                    }
                                } else {
                                    // Create lane in both models
                                    let lane = AutomationLane::new_effect(p.id, p.name.clone(), inst_id);
                                    if let Ok(mut ts) = app.engine.tracks.lock() {
                                        if let Some(t) = ts.get_mut(track_idx) {
                                            t.automation_lanes.push(lane.clone());
                                        }
                                    }
                                    if let Some(track) = app.project.tracks.get_mut(track_idx) {
                                        track.automation_lanes.push(lane);
                                    }
                                }
                                dirty = true;
                            }
                        }
                    });
                }
            }
        });

    app.effect_editor_state.selected_effect = selected;
    app.effect_editor_state.selected_track = Some(track_idx);
    if dirty {
        ctx.request_repaint();
    }
}
