mod eq_graph;

use crate::app::HdawApp;
use crate::audio::effects::create_effect;
use crate::audio::effects::dsp_effect::{EffectInstance, EffectType};
use crate::project::automation::AutomationLane;
use egui::{Color32, Context, SidePanel, Slider, Vec2};

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}
use eq_graph::{FxData, ParamData};

#[derive(Default)]
pub struct EffectEditorState {
    pub selected_track: Option<usize>,
    pub selected_effect: Option<usize>,
    pub show_editor: bool,
    pub show_add_menu: bool,
}

fn read_fx_chain(app: &HdawApp, track_idx: usize) -> (Vec<FxData>, String) {
    let empty = String::new();
    let name = app.track_ui.get(track_idx).map(|t| t.name.clone()).unwrap_or(empty);
    let Ok(tracks) = app.engine.tracks.lock() else { return (Vec::new(), name) };
    let Some(track) = tracks.get(track_idx) else { return (Vec::new(), name) };
    let data = track.fx_chain.iter().map(|inst| FxData {
        type_name: inst.name.clone(),
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
            if let Some(pv) = fx.param_values.get_mut(param_id as usize) {
                *pv = value;
            }
        }
    }
}

fn remove_effect(app: &mut HdawApp, track_idx: usize, effect_idx: usize) {
    app.close_plugin_gui(track_idx, effect_idx);
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
        app.undo_service.push(crate::app::undo::UndoCommand::RemoveEffect {
            track_index: track_idx,
            effect_index: effect_idx,
            serialized: s,
            removed_lanes,
        });
    }
}

fn add_builtin_effect(app: &mut HdawApp, track_idx: usize, name: &str, etype: EffectType) {
    let sr = app.engine.transport.sample_rate();
    let mut effect = create_effect(etype.clone());
    effect.reset(sr);
    let instance = EffectInstance::new_builtin(name.to_string(), etype.clone(), effect);
    let effect_index;
    let serialized;
    if let Ok(mut ts) = app.engine.tracks.lock() {
        if let Some(t) = ts.get_mut(track_idx) {
            effect_index = t.fx_chain.len();
            t.add_effect(instance);
            serialized = t.fx_chain.last().map(engine_fx_to_serialized).unwrap();
        } else { return; }
    } else { return; }
    if let Some(track) = app.project.tracks.get_mut(track_idx) {
        let idx = effect_index.min(track.fx_chain.len());
        track.fx_chain.insert(idx, serialized.clone());
    }
    app.undo_service.push(crate::app::undo::UndoCommand::AddEffect {
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
            serialized = t.fx_chain.last().map(engine_fx_to_serialized).unwrap();
        } else { return; }
    } else { return; }
    if let Some(track) = app.project.tracks.get_mut(track_idx) {
        let idx = effect_index.min(track.fx_chain.len());
        track.fx_chain.insert(idx, serialized.clone());
    }
    app.undo_service.push(crate::app::undo::UndoCommand::AddEffect {
        track_index: track_idx,
        effect_index,
        serialized,
    });
}

const EFFECT_TYPES: [(&str, EffectType); 9] = [
    ("Gain",       EffectType::Gain),
    ("EQ",         EffectType::Equalizer),
    ("Compressor", EffectType::Compressor),
    ("Reverb",     EffectType::Reverb),
    ("Delay",      EffectType::Delay),
    ("Chorus",     EffectType::Chorus),
    ("Flanger",    EffectType::Flanger),
    ("Phaser",     EffectType::Phaser),
    ("Distortion", EffectType::Distortion),
];

pub fn render(ctx: &Context, app: &mut HdawApp) {
    if !app.effect_editor_state.show_editor {
        return;
    }

    let track_idx = match app.effect_editor_state.selected_track.or(app.selected_track) {
        Some(t) if t < app.track_ui.len() => t,
        _ => {
            let panel_res = SidePanel::right("effect_editor")
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
            app.preferences.effect_panel_width = panel_res.response.rect.width();
            return;
        }
    };

    if app.effect_editor_state.show_add_menu {
        egui::Window::new("Select Effect")
            .collapsible(false)
            .resizable(true)
            .default_width(400.0)
            .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    ui.label("Built-in");
                    ui.horizontal_wrapped(|ui| {
                        ui.set_min_height(24.0);
                        for (name, etype) in &EFFECT_TYPES {
                            if ui.button(*name).clicked() {
                                add_builtin_effect(app, track_idx, name, etype.clone());
                                app.effect_editor_state.show_add_menu = false;
                            }
                        }
                    });
                    if !app.plugin_registry.is_empty() {
                        ui.separator();
                        ui.label("CLAP Plugins");
                        ui.horizontal_wrapped(|ui| {
                            ui.set_min_height(24.0);
                            let descriptors = app.plugin_registry.clone();
                            for desc in &descriptors {
                                let label = if desc.is_instrument {
                                    truncate(&format!("{} [instrument]", desc.name), 40)
                                } else {
                                    truncate(&desc.name, 35)
                                };
                                if ui.button(label).clicked() {
                                    add_clap_effect(app, track_idx, desc);
                                    app.effect_editor_state.show_add_menu = false;
                                }
                            }
                        });
                    }
                });
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

    let panel_res = SidePanel::right("effect_editor")
        .resizable(true)
        .default_width(app.preferences.effect_panel_width)
        .min_width(200.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("FX Editor");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("\u{2715}").clicked() { // Cross symbol
                        app.effect_editor_state.show_editor = false;
                    }
                });
            });
            ui.label(egui::RichText::new(format!("Track: {track_name}")).weak());
            ui.separator();

            ui.horizontal_wrapped(|ui| {
                for (ei, fx) in fx_data.iter().enumerate() {
                    let is_selected = selected == Some(ei);
                    let cc = if fx.bypass { Color32::from_gray(60) }
                    else if is_selected { Color32::from_rgb(0x3a, 0x6a, 0xaa) }
                    else { Color32::from_rgb(0x3a, 0x4a, 0x3a) };
                    
                    let btn = egui::Button::new(&fx.type_name)
                        .fill(cc)
                        .min_size(Vec2::new(60.0, 24.0));
                    
                    if ui.add_sized([80.0, 24.0], btn).clicked() {
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
                    ui.label(egui::RichText::new(&fx.type_name).strong());
                    let mut bp = fx.bypass;
                    if ui.checkbox(&mut bp, "Bypass").changed() {
                        write_bypass(app, track_idx, ei, bp);
                        dirty = true;
                        return;
                    }

                    if let EffectType::Clap { .. } = fx.etype {
                        let has_gui = if let Ok(ts) = app.engine.tracks.lock() {
                            ts.get(track_idx).and_then(|t| t.fx_chain.get(ei))
                                .and_then(|inst| match &inst.kind {
                                    crate::audio::effects::dsp_effect::EffectKind::Clap(a) => Some(a.try_lock().map(|l| l.gui_supported).unwrap_or(false)),
                                    _ => None,
                                }).unwrap_or(false)
                        } else {
                            false
                        };

                        if has_gui {
                            let state = app.active_plugin_guis.get(&(track_idx, ei));
                            let active = state.is_some();
                            let is_separate = state.is_some_and(|s| s.separate);

                            ui.horizontal(|ui| {
                                let btn_cc = if active && !is_separate { Color32::from_rgb(0x3a, 0xaa, 0x6a) } else { Color32::from_gray(80) };
                                if ui.add(egui::Button::new("GUI (Embedded)").fill(btn_cc)).clicked() {
                                    app.toggle_plugin_gui(track_idx, ei, false);
                                    dirty = true;
                                }

                                let btn_sep_cc = if active && is_separate { Color32::from_rgb(0x3a, 0xaa, 0x6a) } else { Color32::from_gray(80) };
                                if ui.add(egui::Button::new("GUI (Separate)").fill(btn_sep_cc)).clicked() {
                                    app.toggle_plugin_gui(track_idx, ei, true);
                                    dirty = true;
                                }
                            });
                        }
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
        });

    app.effect_editor_state.selected_effect = selected;
    app.effect_editor_state.selected_track = Some(track_idx);
    app.preferences.effect_panel_width = panel_res.response.rect.width();
    if dirty {
        ctx.request_repaint();
    }
}
