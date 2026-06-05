use crate::audio::buffer::AudioBuffer;
use crate::audio::clap_effect::ClapEffectAdapter;
use crate::audio::effects::create_effect;
use crate::audio::effects::dsp_effect::{EffectInstance, EffectType};
use crate::app::TrackUiState;
use crate::project::clip::ClipKind;
use crate::project::clip_handle::ClipHandle;
use crate::project::midi_note::MidiNote;
use crate::project::track::{SerializedEffect, TrackHandle};
use crate::project::Project;
use std::path::Path;
use std::sync::atomic::Ordering;

// ── Audio resampling ──────────────────────────────────────────────

pub fn resample(samples: &[f32], channels: u16, from_sr: u32, to_sr: u32) -> Vec<f32> {
    let from_frames = samples.len() / channels as usize;
    let to_frames = (from_frames as f64 * to_sr as f64 / from_sr as f64).round() as usize;
    let ratio = from_frames as f64 / to_frames as f64;
    let mut out = Vec::with_capacity(to_frames * channels as usize);
    for i in 0..to_frames {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos as usize;
        let frac = src_pos - src_idx as f64;
        for ch in 0..channels as usize {
            let a = samples
                .get(src_idx * channels as usize + ch)
                .copied()
                .unwrap_or(0.0);
            let b = samples
                .get((src_idx + 1) * channels as usize + ch)
                .copied()
                .unwrap_or(0.0);
            out.push(a + (b - a) * frac as f32);
        }
    }
    out
}

// ── Project I/O ───────────────────────────────────────────────────

pub fn serialize_project(project: &Project) -> Result<String, String> {
    ron::ser::to_string_pretty(project, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("serialize: {e}"))
}

pub fn save_to_file(project: &Project, path: &str) -> Result<(), String> {
    let data = serialize_project(project)?;
    std::fs::write(path, &data).map_err(|e| format!("write: {e}"))
}

pub fn load_from_file(path: &str) -> Result<Project, String> {
    let data = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    let mut project: Project =
        ron::de::from_str(&data).map_err(|e| format!("deserialize: {e}"))?;
    // Reload audio buffers + mark MIDI thumbs dirty
    for track in &mut project.tracks {
        for clip in &mut track.clips {
            match clip {
                ClipKind::Audio(audio_clip) => {
                    let _ = audio_clip.reload_buffer();
                }
                ClipKind::Midi(midi_clip) => {
                    midi_clip.thumb_dirty = true;
                }
            }
        }
    }
    Ok(project)
}

// ── Engine rebuild ────────────────────────────────────────────────

/// Rebuilds engine track handles and UI state from a Project model.
/// Returns both the engine handle list and the UI state list.
pub fn rebuild_engine_handles(
    project: &Project,
    sample_rate: u32,
) -> (Vec<TrackHandle>, Vec<TrackUiState>) {
    let mut engine_tracks = Vec::with_capacity(project.tracks.len());
    let mut track_ui = Vec::with_capacity(project.tracks.len());

    for track in &project.tracks {
        let mut handle = TrackHandle::new();
        handle.id = track.id;

        for clip in &track.clips {
            match clip {
                ClipKind::Audio(audio_clip) => {
                    if let Some(buf) = &audio_clip.buffer {
                        let clip_handle = ClipHandle::new(
                            audio_clip.id,
                            (**buf.samples()).to_vec(),
                            buf.channels(),
                            buf.sample_rate(),
                        );
                        clip_handle.set_position(audio_clip.position_frames);
                        clip_handle.set_offset(audio_clip.offset_frames);
                        clip_handle.set_length(audio_clip.length_frames);
                        clip_handle
                            .fade_in_frames
                            .store(audio_clip.fade_in_frames, Ordering::Release);
                        clip_handle
                            .fade_out_frames
                            .store(audio_clip.fade_out_frames, Ordering::Release);
                        handle.add_clip(clip_handle);
                    }
                }
                ClipKind::Midi(midi_clip) => {
                    let clip_handle = ClipHandle::new_midi(
                        midi_clip.id,
                        midi_clip.notes.clone(),
                        midi_clip.length_frames,
                        sample_rate,
                    );
                    clip_handle.set_position(midi_clip.position_frames);
                    handle.add_clip(clip_handle);
                }
            }
        }

        for sfx in &track.fx_chain {
            let instance = match &sfx.effect_type {
                EffectType::Clap { plugin_id, path } => {
                    let adapter = ClapEffectAdapter::new_instance(
                        plugin_id,
                        Path::new(path),
                        sample_rate,
                    );
                    match adapter {
                        Ok(mut a) => {
                            for (i, val) in sfx.param_values.iter().enumerate() {
                                if let Some(info) = a.parameter_info().get(i) {
                                    a.set_parameter(info.id, *val);
                                }
                            }
                            EffectInstance::new_clap(
                                sfx.name.clone(),
                                sfx.effect_type.clone(),
                                a,
                            )
                        }
                        Err(e) => {
                            tracing::error!("Failed to load CLAP effect {}: {}", plugin_id, e);
                            continue;
                        }
                    }
                }
                _ => {
                    let mut effect = create_effect(sfx.effect_type.clone());
                    for (i, val) in sfx.param_values.iter().enumerate() {
                        if let Some(info) = effect.parameter_info().get(i) {
                            effect.set_parameter(info.id, *val);
                        }
                    }
                    effect.reset(sample_rate);
                    EffectInstance::new_builtin(
                        sfx.name.clone(),
                        sfx.effect_type.clone(),
                        effect,
                    )
                }
            };
            instance.set_bypass(sfx.bypass);
            handle.add_effect(instance);
        }

        handle.parent_group = track.parent_group;
        handle.is_group = track.is_group;
        handle.is_return = track.is_return;
        for sdef in &track.sends {
            handle.sends.push(crate::project::track::SendSlot::new(
                sdef.target_id,
                sdef.level,
                sdef.pre_fader,
            ));
        }

        handle
            .volume
            .store(f32::to_bits(track.volume), Ordering::Release);
        handle
            .pan
            .store(f32::to_bits(track.pan), Ordering::Release);
        handle.mute.store(track.mute, Ordering::Release);
        handle.solo.store(track.solo, Ordering::Release);

        for (li, lane) in track.automation_lanes.iter().enumerate() {
            if let Some(al) = handle.automation_lanes.get_mut(li) {
                al.points.clone_from(&lane.points);
            }
        }

        let ui = TrackUiState {
            id: track.id,
            name: track.name.clone(),
            color: track.color,
            volume: handle.volume.clone(),
            pan: handle.pan.clone(),
            mute: handle.mute.clone(),
            solo: handle.solo.clone(),
            armed: handle.armed.clone(),
            peak_left: handle.peak_left.clone(),
            peak_right: handle.peak_right.clone(),
            parent_group: track.parent_group,
            is_group: track.is_group,
            is_return: track.is_return,
            collapsed: false,
            send_levels: handle.sends.iter().map(|s| s.level.clone()).collect(),
        };

        track_ui.push(ui);
        engine_tracks.push(handle);
    }

    (engine_tracks, track_ui)
}

// ── Sync engine → project ─────────────────────────────────────────

#[allow(clippy::type_complexity)]
pub fn sync_engine_to_project(
    project: &mut Project,
    tracks: &[TrackHandle],
) {
    let snapshot: Vec<(
        f32, f32, bool, bool,
        Vec<Vec<crate::project::automation::AutomationPoint>>,
        Vec<SerializedEffect>,
        Vec<(uuid::Uuid, Vec<MidiNote>)>,
        Vec<(uuid::Uuid, Vec<crate::project::cc_event::CCEvent>)>,
        Option<uuid::Uuid>, bool, bool,
        Vec<crate::project::track::SendSlotDef>,
    )> = tracks.iter().map(|handle| {
        let vol = f32::from_bits(handle.volume.load(Ordering::Acquire));
        let pan = f32::from_bits(handle.pan.load(Ordering::Acquire));
        let mute = handle.mute.load(Ordering::Acquire);
        let solo = handle.solo.load(Ordering::Acquire);
        let auto_points: Vec<Vec<crate::project::automation::AutomationPoint>> = handle
            .automation_lanes.iter().map(|l| l.points.clone()).collect();
        let fx: Vec<SerializedEffect> = handle.fx_chain.iter().map(|inst| {
            let pv: Vec<f32> = inst.parameter_info().iter()
                .map(|p| inst.parameter_value(p.id))
                .collect();
            SerializedEffect {
                name: inst.name.clone(),
                effect_type: inst.effect_type.clone(),
                bypass: inst.is_bypassed(),
                param_values: pv,
            }
        }).collect();
        let clip_notes: Vec<(uuid::Uuid, Vec<MidiNote>)> = handle.clips.iter()
            .map(|c| (c.clip_id, c.midi_notes.clone()))
            .collect();
        let clip_cc: Vec<(uuid::Uuid, Vec<crate::project::cc_event::CCEvent>)> = handle.clips.iter()
            .map(|c| (c.clip_id, c.midi_cc_events.clone()))
            .collect();
        let sends: Vec<crate::project::track::SendSlotDef> = handle.sends.iter().map(|s| {
            crate::project::track::SendSlotDef {
                target_id: s.target_id,
                level: f32::from_bits(s.level.load(Ordering::Acquire)),
                pre_fader: s.pre_fader,
            }
        }).collect();
        (vol, pan, mute, solo, auto_points, fx, clip_notes, clip_cc, handle.parent_group, handle.is_group, handle.is_return, sends)
    }).collect();

    for (ti, (vol, pan, mute, solo, auto_points, fx, clip_notes, clip_cc, parent_group, is_group, is_return, sends)) in snapshot.into_iter().enumerate() {
        if let Some(track) = project.tracks.get_mut(ti) {
            track.volume = vol;
            track.pan = pan;
            track.mute = mute;
            track.solo = solo;
            track.parent_group = parent_group;
            track.is_group = is_group;
            track.is_return = is_return;
            track.sends = sends;
            for (li, points) in auto_points.into_iter().enumerate() {
                if let Some(lane) = track.automation_lanes.get_mut(li) {
                    if lane.points != points {
                        lane.points = points;
                    }
                }
            }
            track.fx_chain = fx;
            for (clip_id, notes) in clip_notes {
                if let Some(ClipKind::Midi(m)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                    if m.notes != notes {
                        m.notes = notes;
                        m.thumb_dirty = true;
                    }
                }
            }
            for (clip_id, cc_events) in clip_cc {
                if let Some(ClipKind::Midi(m)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                    if m.cc_events != cc_events {
                        m.cc_events = cc_events;
                    }
                }
            }
        }
    }

    // Sync fade values
    let fade_snapshot: Vec<(uuid::Uuid, u64, u64)> = tracks.iter().flat_map(|handle| {
        handle.clips.iter().map(|ch| {
            (ch.clip_id, ch.fade_in_frames.load(Ordering::Acquire), ch.fade_out_frames.load(Ordering::Acquire))
        }).collect::<Vec<_>>()
    }).collect();
    for (clip_id, fi, fo) in fade_snapshot {
        for track in project.tracks.iter_mut() {
            if let Some(ClipKind::Audio(a)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Audio(ca) if ca.id == clip_id)) {
                a.fade_in_frames = fi;
                a.fade_out_frames = fo;
                break;
            }
        }
    }
}

// ── Audio file loading ────────────────────────────────────────────

/// Loads a WAV file, resampling to the target sample rate if needed.
pub fn load_audio_file_resampled(
    path: &str,
    engine_sr: u32,
) -> Result<(AudioBuffer, String), String> {
    let (samples, channels, file_sr) = crate::utils::load_wav_file(path)?;
    let samples = if file_sr != engine_sr {
        resample(&samples, channels, file_sr, engine_sr)
    } else {
        samples
    };
    let file_name = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();
    let buffer = AudioBuffer::from_interleaved(samples, channels, engine_sr);
    Ok((buffer, file_name))
}
