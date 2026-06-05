use crate::app::undo::UndoCommand;
use crate::app::HdawApp;
use crate::audio::effects::dsp_effect::{EffectInstance, EffectType};

impl HdawApp {
    pub fn assign_instrument(&mut self, track_index: usize, desc: &crate::audio::clap_scanner::PluginDescriptor) {
        let sr = self.engine.transport.sample_rate();
        let adapter = match crate::audio::clap_effect::ClapEffectAdapter::new_instance(&desc.id, &desc.path, sr) {
            Ok(a) => a,
            Err(e) => {
                self.error_message = Some(format!("Failed to load instrument {}: {}", desc.name, e));
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
        if let Ok(mut ts) = self.engine.tracks.lock() {
            if let Some(t) = ts.get_mut(track_index) {
                effect_index = t.fx_chain.len();
                t.add_effect(instance);
                let inst = t.fx_chain.last().unwrap();
                let pv: Vec<f32> = inst.parameter_info().iter()
                    .map(|p| inst.parameter_value(p.id)).collect();
                serialized = crate::project::track::SerializedEffect {
                    name: inst.name.clone(),
                    effect_type: inst.effect_type.clone(),
                    bypass: inst.is_bypassed(),
                    param_values: pv,
                };
            } else { return; }
        } else { return; }

        if let Some(track) = self.project.tracks.get_mut(track_index) {
            let idx = effect_index.min(track.fx_chain.len());
            track.fx_chain.insert(idx, serialized.clone());
        }

        self.undo_service.push(UndoCommand::AddEffect {
            track_index,
            effect_index,
            serialized,
        });
    }

    pub fn replace_instrument(&mut self, track_index: usize, desc: &crate::audio::clap_scanner::PluginDescriptor) {
        let old_inst_idx: Option<usize>;
        let old_serialized: Option<crate::project::track::SerializedEffect>;
        let old_undo: Option<UndoCommand>;

        if let Ok(mut ts) = self.engine.tracks.lock() {
            if let Some(t) = ts.get_mut(track_index) {
                old_inst_idx = t.fx_chain.iter().position(|e| e.has_note_input);
                if let Some(idx) = old_inst_idx {
                    let inst = &t.fx_chain[idx];
                    let pv: Vec<f32> = inst.parameter_info().iter()
                        .map(|p| inst.parameter_value(p.id)).collect();
                    old_serialized = Some(crate::project::track::SerializedEffect {
                        name: inst.name.clone(),
                        effect_type: inst.effect_type.clone(),
                        bypass: inst.is_bypassed(),
                        param_values: pv,
                    });
                    old_undo = Some(UndoCommand::RemoveEffect {
                        track_index,
                        effect_index: idx,
                        serialized: old_serialized.clone().unwrap(),
                        removed_lanes: Vec::new(),
                    });
                    t.fx_chain.remove(idx);
                } else {
                    old_serialized = None;
                    old_undo = None;
                }
            } else {
                return;
            }
        } else {
            return;
        }

        if let (Some(idx), Some(_)) = (old_inst_idx, &old_serialized) {
            if let Some(track) = self.project.tracks.get_mut(track_index) {
                if idx < track.fx_chain.len() {
                    track.fx_chain.remove(idx);
                }
            }
            self.undo_service.push(old_undo.unwrap());
        }

        let sr = self.engine.transport.sample_rate();
        let adapter = match crate::audio::clap_effect::ClapEffectAdapter::new_instance(&desc.id, &desc.path, sr) {
            Ok(a) => a,
            Err(e) => {
                self.error_message = Some(format!("Failed to load instrument {}: {}", desc.name, e));
                return;
            }
        };
        let etype = EffectType::Clap {
            plugin_id: desc.id.clone(),
            path: desc.path.to_string_lossy().into_owned(),
        };
        let instance = EffectInstance::new_clap(desc.name.clone(), etype.clone(), adapter);

        let insert_at = old_inst_idx.unwrap_or(0);
        let effect_index;
        let serialized;
        if let Ok(mut ts) = self.engine.tracks.lock() {
            if let Some(t) = ts.get_mut(track_index) {
                t.add_effect(instance);
                let idx = t.fx_chain.len() - 1;
                if idx != insert_at {
                    t.fx_chain.swap(idx, insert_at);
                }
                effect_index = insert_at;
                let inst = &t.fx_chain[insert_at];
                let pv: Vec<f32> = inst.parameter_info().iter()
                    .map(|p| inst.parameter_value(p.id)).collect();
                serialized = crate::project::track::SerializedEffect {
                    name: inst.name.clone(),
                    effect_type: inst.effect_type.clone(),
                    bypass: inst.is_bypassed(),
                    param_values: pv,
                };
            } else {
                return;
            }
        } else {
            return;
        }

        if let Some(track) = self.project.tracks.get_mut(track_index) {
            let idx = effect_index.min(track.fx_chain.len());
            track.fx_chain.insert(idx, serialized.clone());
        }

        self.undo_service.push(UndoCommand::AddEffect {
            track_index,
            effect_index,
            serialized,
        });
    }
}
