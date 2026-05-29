use crate::audio::effects::create_effect;
use crate::audio::effects::dsp_effect::EffectInstance;
use crate::project::automation::AutomationPoint;
use crate::project::clip_handle::ClipHandle;
use crate::project::track::TrackHandle;
use crate::project::Project;
use std::sync::atomic::Ordering;

use super::UndoCommand;

fn set_mute(tracks: &mut [TrackHandle], project: &mut Project, track_index: usize, value: bool) {
    if let Some(handle) = tracks.get_mut(track_index) {
        handle.mute.store(value, Ordering::Release);
    }
    if let Some(track) = project.tracks.get_mut(track_index) {
        track.mute = value;
    }
}

fn set_solo(tracks: &mut [TrackHandle], project: &mut Project, track_index: usize, value: bool) {
    if let Some(handle) = tracks.get_mut(track_index) {
        handle.solo.store(value, Ordering::Release);
    }
    if let Some(track) = project.tracks.get_mut(track_index) {
        track.solo = value;
    }
}

fn insert_point(tracks: &mut [TrackHandle], project: &mut Project, track_index: usize, lane_index: usize, point: &AutomationPoint) {
    if let Some(handle) = tracks.get_mut(track_index) {
        if let Some(lane) = handle.automation_lanes.get_mut(lane_index) {
            let idx = lane.points.iter().position(|p| p.time_frames == point.time_frames).unwrap_or(lane.points.len());
            lane.points.insert(idx, point.clone());
        }
    }
    if let Some(track) = project.tracks.get_mut(track_index) {
        if let Some(lane) = track.automation_lanes.get_mut(lane_index) {
            let idx = lane.points.iter().position(|p| p.time_frames == point.time_frames).unwrap_or(lane.points.len());
            lane.points.insert(idx, point.clone());
        }
    }
}

fn remove_point(tracks: &mut [TrackHandle], project: &mut Project, track_index: usize, lane_index: usize, point: &AutomationPoint) {
    if let Some(handle) = tracks.get_mut(track_index) {
        if let Some(lane) = handle.automation_lanes.get_mut(lane_index) {
            if let Some(idx) = lane.points.iter().position(|p| p.time_frames == point.time_frames && p.value == point.value) {
                lane.points.remove(idx);
            }
        }
    }
    if let Some(track) = project.tracks.get_mut(track_index) {
        if let Some(lane) = track.automation_lanes.get_mut(lane_index) {
            if let Some(idx) = lane.points.iter().position(|p| p.time_frames == point.time_frames && p.value == point.value) {
                lane.points.remove(idx);
            }
        }
    }
}

fn recreate_effect(serialized: &crate::project::track::SerializedEffect) -> EffectInstance {
    let effect = create_effect(serialized.effect_type);
    for (i, val) in serialized.param_values.iter().enumerate() {
        if let Some(info) = effect.parameter_info().get(i) {
            effect.set_parameter(info.id, *val);
        }
    }
    let inst = EffectInstance::new(serialized.name.clone(), serialized.effect_type, effect);
    inst.set_bypass(serialized.bypass);
    inst
}

pub fn apply_undo(project: &mut Project, tracks: &mut [TrackHandle], cmd: &UndoCommand) {
    match *cmd {
        UndoCommand::MoveClip { track_index, clip_id, old_position, .. } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
                    clip.position_frames = old_position;
                }
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter().find(|c| c.clip_id == clip_id) {
                    ch.set_position(old_position);
                }
            }
        }
        UndoCommand::TrimClip { track_index, clip_id, old_offset, old_length, .. } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
                    clip.offset_frames = old_offset;
                    clip.length_frames = old_length;
                }
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter().find(|c| c.clip_id == clip_id) {
                    ch.set_offset(old_offset);
                    ch.set_length(old_length);
                }
            }
        }
        UndoCommand::DeleteClip { track_index, clip_index: _, ref clip } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                let pos = track.clips.iter().position(|c| c.id == clip.id)
                    .unwrap_or(track.clips.len());
                if let Some(buf) = clip.buffer.as_ref() {
                    let ch = ClipHandle::new(clip.id, (**buf.samples()).clone(), buf.channels(), buf.sample_rate());
                    ch.set_position(clip.position_frames);
                    ch.set_offset(clip.offset_frames);
                    ch.set_length(clip.length_frames);
                    if let Some(handle) = tracks.get_mut(track_index) {
                        let eng_pos = handle.find_clip_by_id(clip.id).unwrap_or(handle.clips.len());
                        handle.clips.insert(eng_pos, ch);
                    }
                }
                track.clips.insert(pos, clip.clone());
            }
        }
        UndoCommand::AddEffect { track_index, effect_index, .. } => {
            if let Some(handle) = tracks.get_mut(track_index) {
                if effect_index < handle.fx_chain.len() { handle.fx_chain.remove(effect_index); }
            }
            if let Some(track) = project.tracks.get_mut(track_index) {
                if effect_index < track.fx_chain.len() { track.fx_chain.remove(effect_index); }
            }
        }
        UndoCommand::RemoveEffect { track_index, effect_index, ref serialized } => {
            if let Some(handle) = tracks.get_mut(track_index) {
                let idx = effect_index.min(handle.fx_chain.len());
                handle.fx_chain.insert(idx, recreate_effect(serialized));
            }
            if let Some(track) = project.tracks.get_mut(track_index) {
                let idx = effect_index.min(track.fx_chain.len());
                track.fx_chain.insert(idx, serialized.clone());
            }
        }
        UndoCommand::ToggleMute { track_index, old_value } => set_mute(tracks, project, track_index, old_value),
        UndoCommand::ToggleSolo { track_index, old_value } => set_solo(tracks, project, track_index, old_value),
        UndoCommand::AutomationAddPoint { track_index, lane_index, ref point } => {
            remove_point(tracks, project, track_index, lane_index, point);
        }
        UndoCommand::AutomationRemovePoint { track_index, lane_index, point_index: _, ref point } => {
            insert_point(tracks, project, track_index, lane_index, point);
        }
        UndoCommand::AutomationMovePoint { track_index, lane_index, point_index, old_value, .. } => {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(lane) = handle.automation_lanes.get_mut(lane_index) {
                    if let Some(pt) = lane.points.get_mut(point_index) { pt.value = old_value; }
                }
            }
            if let Some(track) = project.tracks.get_mut(track_index) {
                if let Some(lane) = track.automation_lanes.get_mut(lane_index) {
                    if let Some(pt) = lane.points.get_mut(point_index) { pt.value = old_value; }
                }
            }
        }
    }
}

pub fn apply_redo(project: &mut Project, tracks: &mut [TrackHandle], cmd: &UndoCommand) {
    match *cmd {
        UndoCommand::MoveClip { track_index, clip_id, new_position, .. } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
                    clip.position_frames = new_position;
                }
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter().find(|c| c.clip_id == clip_id) {
                    ch.set_position(new_position);
                }
            }
        }
        UndoCommand::TrimClip { track_index, clip_id, new_offset, new_length, .. } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
                    clip.offset_frames = new_offset;
                    clip.length_frames = new_length;
                }
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter().find(|c| c.clip_id == clip_id) {
                    ch.set_offset(new_offset);
                    ch.set_length(new_length);
                }
            }
        }
        UndoCommand::DeleteClip { track_index, clip_index: _, ref clip } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                if let Some(pos) = track.clips.iter().position(|c| c.id == clip.id) { track.clips.remove(pos); }
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(pos) = handle.find_clip_by_id(clip.id) { handle.clips.remove(pos); }
            }
        }
        UndoCommand::AddEffect { .. } => {}
        UndoCommand::RemoveEffect { track_index, effect_index, ref serialized } => {
            if let Some(handle) = tracks.get_mut(track_index) {
                let idx = effect_index.min(handle.fx_chain.len());
                handle.fx_chain.insert(idx, recreate_effect(serialized));
            }
            if let Some(track) = project.tracks.get_mut(track_index) {
                let idx = effect_index.min(track.fx_chain.len());
                track.fx_chain.insert(idx, serialized.clone());
            }
        }
        UndoCommand::ToggleMute { track_index, old_value } => set_mute(tracks, project, track_index, !old_value),
        UndoCommand::ToggleSolo { track_index, old_value } => set_solo(tracks, project, track_index, !old_value),
        UndoCommand::AutomationAddPoint { track_index, lane_index, ref point } => {
            insert_point(tracks, project, track_index, lane_index, point);
        }
        UndoCommand::AutomationRemovePoint { track_index, lane_index, point_index: _, ref point } => {
            remove_point(tracks, project, track_index, lane_index, point);
        }
        UndoCommand::AutomationMovePoint { track_index, lane_index, point_index, new_value, .. } => {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(lane) = handle.automation_lanes.get_mut(lane_index) {
                    if let Some(pt) = lane.points.get_mut(point_index) { pt.value = new_value; }
                }
            }
            if let Some(track) = project.tracks.get_mut(track_index) {
                if let Some(lane) = track.automation_lanes.get_mut(lane_index) {
                    if let Some(pt) = lane.points.get_mut(point_index) { pt.value = new_value; }
                }
            }
        }
    }
}
