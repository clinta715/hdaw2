use crate::audio::clap_effect::ClapEffectAdapter;
use crate::audio::effects::create_effect;
use crate::audio::effects::dsp_effect::{EffectInstance, EffectType};
use crate::project::automation::AutomationPoint;
use crate::project::cc_event::CCEvent;
use crate::project::clip::ClipKind;
use crate::project::clip_handle::ClipHandle;
use crate::project::track::TrackHandle;
use crate::project::Project;
use std::path::Path;
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

fn recreate_effect(serialized: &crate::project::track::SerializedEffect, sample_rate: u32) -> EffectInstance {
    match &serialized.effect_type {
        EffectType::Clap { plugin_id, path } => {
            match ClapEffectAdapter::new_instance(plugin_id, Path::new(path), sample_rate) {
                Ok(adapter) => {
                    let inst = EffectInstance::new_clap(
                        serialized.name.clone(),
                        serialized.effect_type.clone(),
                        adapter,
                    );
                    inst.set_bypass(serialized.bypass);
                    inst
                }
                Err(e) => {
                    tracing::error!("Failed to recreate CLAP effect {}: {}", plugin_id, e);
                    let effect = create_effect(EffectType::Gain);
                    let inst = EffectInstance::new_builtin(
                        serialized.name.clone(),
                        serialized.effect_type.clone(),
                        effect,
                    );
                    inst.set_bypass(serialized.bypass);
                    inst
                }
            }
        }
        _ => {
            let effect = create_effect(serialized.effect_type.clone());
            for (i, val) in serialized.param_values.iter().enumerate() {
                if let Some(info) = effect.parameter_info().get(i) {
                    effect.set_parameter(info.id, *val);
                }
            }
            let inst = EffectInstance::new_builtin(serialized.name.clone(), serialized.effect_type.clone(), effect);
            inst.set_bypass(serialized.bypass);
            inst
        }
    }
}

fn update_note(tracks: &mut [TrackHandle], project: &mut Project, track_index: usize, clip_id: uuid::Uuid, note_index: usize, note: &crate::project::midi_note::MidiNote) {
    if let Some(track) = project.tracks.get_mut(track_index) {
        if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
            if note_index < clip.notes.len() {
                clip.notes[note_index] = note.clone();
                clip.notes.sort_by_key(|n| n.start_frame);
                clip.thumb_dirty = true;
            }
        }
    }
    if let Some(handle) = tracks.get_mut(track_index) {
        if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
            if note_index < ch.midi_notes.len() {
                ch.midi_notes[note_index] = note.clone();
                ch.midi_notes.sort_by_key(|n| n.start_frame);
            }
        }
    }
}

fn add_cc_event(tracks: &mut [TrackHandle], project: &mut Project, track_index: usize, clip_id: uuid::Uuid, event: &CCEvent) {
    if let Some(track) = project.tracks.get_mut(track_index) {
        if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
            clip.cc_events.push(event.clone());
            clip.cc_events.sort_by_key(|e| e.time_frames);
        }
    }
    if let Some(handle) = tracks.get_mut(track_index) {
        if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
            ch.midi_cc_events.push(event.clone());
            ch.midi_cc_events.sort_by_key(|e| e.time_frames);
        }
    }
}

fn remove_cc_event(tracks: &mut [TrackHandle], project: &mut Project, track_index: usize, clip_id: uuid::Uuid, event: &CCEvent) {
    if let Some(track) = project.tracks.get_mut(track_index) {
        if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
            clip.cc_events.retain(|e| e.cc_number != event.cc_number || e.time_frames != event.time_frames);
        }
    }
    if let Some(handle) = tracks.get_mut(track_index) {
        if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
            ch.midi_cc_events.retain(|e| e.cc_number != event.cc_number || e.time_frames != event.time_frames);
        }
    }
}

fn replace_cc_event(tracks: &mut [TrackHandle], project: &mut Project, track_index: usize, clip_id: uuid::Uuid, _event_index: usize, event: &CCEvent) {
    if let Some(track) = project.tracks.get_mut(track_index) {
        if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
            if let Some(ctx) = clip.cc_events.iter_mut().find(|e| e.cc_number == event.cc_number && e.time_frames == event.time_frames) {
                *ctx = event.clone();
            }
        }
    }
    if let Some(handle) = tracks.get_mut(track_index) {
        if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
            if let Some(ctx) = ch.midi_cc_events.iter_mut().find(|e| e.cc_number == event.cc_number && e.time_frames == event.time_frames) {
                *ctx = event.clone();
            }
        }
    }
}

pub fn apply_undo(project: &mut Project, tracks: &mut [TrackHandle], cmd: &UndoCommand, sample_rate: u32) {
    match *cmd {
        UndoCommand::UpdateMidiNote { track_index, clip_id, note_index, ref old_note, .. } => {
            update_note(tracks, project, track_index, clip_id, note_index, old_note);
        }
        UndoCommand::AddCcEvent { track_index, clip_id, ref event } => {
            remove_cc_event(tracks, project, track_index, clip_id, event);
        }
        UndoCommand::RemoveCcEvent { track_index, clip_id, ref event, .. } => {
            add_cc_event(tracks, project, track_index, clip_id, event);
        }
        UndoCommand::MoveCcEvent { track_index, clip_id, event_index, ref old_event, .. } => {
            replace_cc_event(tracks, project, track_index, clip_id, event_index, old_event);
        }
        UndoCommand::MoveClip { track_index, clip_id, old_position, .. } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                for clip in track.clips.iter_mut() {
                    match clip {
                        ClipKind::Audio(a) if a.id == clip_id => a.position_frames = old_position,
                        ClipKind::Midi(m) if m.id == clip_id => m.position_frames = old_position,
                        _ => continue,
                    }
                    break;
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
                for clip in track.clips.iter_mut() {
                    match clip {
                        ClipKind::Audio(a) if a.id == clip_id => {
                            a.offset_frames = old_offset;
                            a.length_frames = old_length;
                        }
                        ClipKind::Midi(m) if m.id == clip_id => {
                            m.length_frames = old_length;
                        }
                        _ => continue,
                    }
                    break;
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
                let clip_id = match clip {
                    ClipKind::Audio(a) => a.id,
                    ClipKind::Midi(m) => m.id,
                };
                let pos = track.clips.iter().position(|c| match c {
                    ClipKind::Audio(a) => a.id == clip_id,
                    ClipKind::Midi(m) => m.id == clip_id,
                }).unwrap_or(track.clips.len());
                match clip {
                    ClipKind::Audio(audio_clip) => {
                        if let Some(buf) = audio_clip.buffer.as_ref() {
                            let ch = ClipHandle::new(audio_clip.id, (**buf.samples()).clone(), buf.channels(), buf.sample_rate());
                            ch.set_position(audio_clip.position_frames);
                            ch.set_offset(audio_clip.offset_frames);
                            ch.set_length(audio_clip.length_frames);
                            if let Some(handle) = tracks.get_mut(track_index) {
                                let eng_pos = handle.find_clip_by_id(audio_clip.id).unwrap_or(handle.clips.len());
                                handle.clips.insert(eng_pos, ch);
                            }
                        }
                    }
                    ClipKind::Midi(midi_clip) => {
                        let ch = ClipHandle::new_midi(midi_clip.id, midi_clip.notes.clone(), midi_clip.length_frames, sample_rate);
                        ch.set_position(midi_clip.position_frames);
                        if let Some(handle) = tracks.get_mut(track_index) {
                            let eng_pos = handle.find_clip_by_id(midi_clip.id).unwrap_or(handle.clips.len());
                            handle.clips.insert(eng_pos, ch);
                        }
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
        UndoCommand::RemoveEffect { track_index, effect_index, ref serialized, ref removed_lanes } => {
            if let Some(handle) = tracks.get_mut(track_index) {
                let idx = effect_index.min(handle.fx_chain.len());
                handle.fx_chain.insert(idx, recreate_effect(serialized, sample_rate));
                for lane in removed_lanes {
                    if !handle.automation_lanes.iter().any(|l| l.effect_instance_id == lane.effect_instance_id && l.param_id == lane.param_id) {
                        handle.automation_lanes.push(lane.clone());
                    }
                }
            }
            if let Some(track) = project.tracks.get_mut(track_index) {
                let idx = effect_index.min(track.fx_chain.len());
                track.fx_chain.insert(idx, serialized.clone());
                for lane in removed_lanes {
                    if !track.automation_lanes.iter().any(|l| l.effect_instance_id == lane.effect_instance_id && l.param_id == lane.param_id) {
                        track.automation_lanes.push(lane.clone());
                    }
                }
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
        UndoCommand::AddMidiNote { track_index, clip_id, ref note } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                    clip.notes.retain(|n| n.pitch != note.pitch || n.start_frame != note.start_frame || n.duration != note.duration);
                    clip.thumb_dirty = true;
                }
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    ch.midi_notes.retain(|n| n.pitch != note.pitch || n.start_frame != note.start_frame || n.duration != note.duration);
                }
            }
        }
        UndoCommand::RemoveMidiNote { track_index, clip_id, ref note, .. } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                    clip.notes.push(note.clone());
                    clip.notes.sort_by_key(|n| n.start_frame);
                    clip.thumb_dirty = true;
                }
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    ch.midi_notes.push(note.clone());
                    ch.midi_notes.sort_by_key(|n| n.start_frame);
                }
            }
        }
        UndoCommand::FadeClip { track_index, clip_id, old_fade_in, old_fade_out, .. } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                for clip in track.clips.iter_mut() {
                    match clip {
                        ClipKind::Audio(a) if a.id == clip_id => {
                            a.fade_in_frames = old_fade_in;
                            a.fade_out_frames = old_fade_out;
                        }
                        _ => continue,
                    }
                    break;
                }
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter().find(|c| c.clip_id == clip_id) {
                    ch.fade_in_frames.store(old_fade_in, Ordering::Release);
                    ch.fade_out_frames.store(old_fade_out, Ordering::Release);
                }
            }
        }
        UndoCommand::AddMidiClip { track_index, ref clip } => {
            let clip_id = match clip {
                ClipKind::Audio(a) => a.id,
                ClipKind::Midi(m) => m.id,
            };
            if let Some(track) = project.tracks.get_mut(track_index) {
                if let Some(pos) = track.clips.iter().position(|c| match c {
                    ClipKind::Audio(a) => a.id == clip_id,
                    ClipKind::Midi(m) => m.id == clip_id,
                }) { track.clips.remove(pos); }
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(pos) = handle.find_clip_by_id(clip_id) { handle.clips.remove(pos); }
            }
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
        UndoCommand::ImportAudio { .. } | UndoCommand::ImportMidi { .. }
                 | UndoCommand::RecordAudio { .. }
         | UndoCommand::AddTrack { .. } | UndoCommand::DeleteTrack { .. } => {}
    }
}

pub fn apply_redo(project: &mut Project, tracks: &mut [TrackHandle], cmd: &UndoCommand, sample_rate: u32) {
    match *cmd {
        UndoCommand::UpdateMidiNote { track_index, clip_id, note_index, ref new_note, .. } => {
            update_note(tracks, project, track_index, clip_id, note_index, new_note);
        }
        UndoCommand::AddCcEvent { track_index, clip_id, ref event } => {
            add_cc_event(tracks, project, track_index, clip_id, event);
        }
        UndoCommand::RemoveCcEvent { track_index, clip_id, ref event, .. } => {
            remove_cc_event(tracks, project, track_index, clip_id, event);
        }
        UndoCommand::MoveCcEvent { track_index, clip_id, event_index, ref new_event, .. } => {
            replace_cc_event(tracks, project, track_index, clip_id, event_index, new_event);
        }
        UndoCommand::MoveClip { track_index, clip_id, new_position, .. } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                for clip in track.clips.iter_mut() {
                    match clip {
                        ClipKind::Audio(a) if a.id == clip_id => a.position_frames = new_position,
                        ClipKind::Midi(m) if m.id == clip_id => m.position_frames = new_position,
                        _ => continue,
                    }
                    break;
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
                for clip in track.clips.iter_mut() {
                    match clip {
                        ClipKind::Audio(a) if a.id == clip_id => {
                            a.offset_frames = new_offset;
                            a.length_frames = new_length;
                        }
                        ClipKind::Midi(m) if m.id == clip_id => {
                            m.length_frames = new_length;
                        }
                        _ => continue,
                    }
                    break;
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
            let clip_id = match clip {
                ClipKind::Audio(a) => a.id,
                ClipKind::Midi(m) => m.id,
            };
            if let Some(track) = project.tracks.get_mut(track_index) {
                if let Some(pos) = track.clips.iter().position(|c| match c {
                    ClipKind::Audio(a) => a.id == clip_id,
                    ClipKind::Midi(m) => m.id == clip_id,
                }) { track.clips.remove(pos); }
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(pos) = handle.find_clip_by_id(clip_id) { handle.clips.remove(pos); }
            }
        }
        UndoCommand::AddEffect { track_index, effect_index, ref serialized } => {
            if let Some(handle) = tracks.get_mut(track_index) {
                let idx = effect_index.min(handle.fx_chain.len());
                handle.fx_chain.insert(idx, recreate_effect(serialized, sample_rate));
            }
            if let Some(track) = project.tracks.get_mut(track_index) {
                let idx = effect_index.min(track.fx_chain.len());
                track.fx_chain.insert(idx, serialized.clone());
            }
        }
        UndoCommand::RemoveEffect { track_index, effect_index, ref removed_lanes, .. } => {
            if let Some(handle) = tracks.get_mut(track_index) {
                if effect_index < handle.fx_chain.len() {
                    handle.fx_chain.remove(effect_index);
                }
                for lane in removed_lanes {
                    handle.automation_lanes.retain(|l|
                        !(l.effect_instance_id == lane.effect_instance_id && l.param_id == lane.param_id)
                    );
                }
            }
            if let Some(track) = project.tracks.get_mut(track_index) {
                if effect_index < track.fx_chain.len() {
                    track.fx_chain.remove(effect_index);
                }
                for lane in removed_lanes {
                    track.automation_lanes.retain(|l|
                        !(l.effect_instance_id == lane.effect_instance_id && l.param_id == lane.param_id)
                    );
                }
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
        UndoCommand::AddMidiNote { track_index, clip_id, ref note } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                    clip.notes.push(note.clone());
                    clip.notes.sort_by_key(|n| n.start_frame);
                    clip.thumb_dirty = true;
                }
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    ch.midi_notes.push(note.clone());
                    ch.midi_notes.sort_by_key(|n| n.start_frame);
                }
            }
        }
        UndoCommand::RemoveMidiNote { track_index, clip_id, ref note, .. } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                    clip.notes.retain(|n| n.pitch != note.pitch || n.start_frame != note.start_frame || n.duration != note.duration);
                    clip.thumb_dirty = true;
                }
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    ch.midi_notes.retain(|n| n.pitch != note.pitch || n.start_frame != note.start_frame || n.duration != note.duration);
                }
            }
        }
        UndoCommand::FadeClip { track_index, clip_id, new_fade_in, new_fade_out, .. } => {
            if let Some(track) = project.tracks.get_mut(track_index) {
                for clip in track.clips.iter_mut() {
                    match clip {
                        ClipKind::Audio(a) if a.id == clip_id => {
                            a.fade_in_frames = new_fade_in;
                            a.fade_out_frames = new_fade_out;
                        }
                        _ => continue,
                    }
                    break;
                }
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter().find(|c| c.clip_id == clip_id) {
                    ch.fade_in_frames.store(new_fade_in, Ordering::Release);
                    ch.fade_out_frames.store(new_fade_out, Ordering::Release);
                }
            }
        }
        UndoCommand::AddMidiClip { track_index, ref clip } => {
            let clip_id = match clip {
                ClipKind::Audio(a) => a.id,
                ClipKind::Midi(m) => m.id,
            };
            let pos = project.tracks.get(track_index).and_then(|t| {
                t.clips.iter().position(|c| match c {
                    ClipKind::Audio(a) => a.id == clip_id,
                    ClipKind::Midi(m) => m.id == clip_id,
                })
            }).unwrap_or(0);
            if let Some(track) = project.tracks.get_mut(track_index) {
                let idx = pos.min(track.clips.len());
                track.clips.insert(idx, clip.clone());
            }
            if let Some(handle) = tracks.get_mut(track_index) {
                if let ClipKind::Midi(midi_clip) = clip {
                    let ch = ClipHandle::new_midi(midi_clip.id, midi_clip.notes.clone(), midi_clip.length_frames, sample_rate);
                    ch.set_position(midi_clip.position_frames);
                    let idx = pos.min(handle.clips.len());
                    handle.clips.insert(idx, ch);
                }
            }
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
        UndoCommand::ImportAudio { .. } | UndoCommand::ImportMidi { .. }
        | UndoCommand::RecordAudio { .. }
        | UndoCommand::AddTrack { .. } | UndoCommand::DeleteTrack { .. } => {}
    }
}
