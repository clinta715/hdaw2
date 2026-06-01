use crate::audio::effects::dsp_effect::EffectKind;
use crate::project::automation::{AutomationLane, PARAM_PAN, PARAM_VOLUME};
use crate::project::track::TrackHandle;
use clack_host::events::event_types::{NoteOffEvent, NoteOnEvent};
use clack_host::events::Pckn;
use clack_host::events::io::EventBuffer;
use std::cell::RefCell;
use std::sync::atomic::Ordering;

thread_local! {
    pub static MIX_L: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    pub static MIX_R: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    pub static PRE_FADER_L: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    pub static PRE_FADER_R: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    static MIDI_EVENTS: RefCell<EventBuffer> = RefCell::new(EventBuffer::with_capacity(128));
}

fn compute_fade_gain(local_frame: usize, clip_len: usize, fade_in: usize, fade_out: usize) -> f32 {
    if fade_in > 0 && local_frame < fade_in {
        return (local_frame as f32 / fade_in as f32).min(1.0);
    }
    if fade_out > 0 {
        let dist_from_end = clip_len.saturating_sub(local_frame);
        if dist_from_end <= fade_out {
            return (dist_from_end as f32 / fade_out as f32).max(0.0);
        }
    }
    1.0
}

fn automation_value(lanes: &[AutomationLane], param_id: u32, time_frames: u64, fallback: f32) -> f32 {
    for lane in lanes {
        if lane.param_id == param_id && !lane.is_empty() {
            let v = lane.get_value_at(time_frames);
            if !v.is_nan() {
                return v;
            }
        }
    }
    fallback
}

pub fn process_track(
    handle: &mut TrackHandle,
    pos: usize,
    frames: usize,
    sample_rate: u32,
    seek_occurred: bool,
) {
    let pos_frames = pos as u64;
    let manual_vol = f32::from_bits(handle.volume.load(Ordering::Acquire));
    let manual_pan = f32::from_bits(handle.pan.load(Ordering::Acquire));

    let track_vol = automation_value(&handle.automation_lanes, PARAM_VOLUME, pos_frames, manual_vol);
    let pan = automation_value(&handle.automation_lanes, PARAM_PAN, pos_frames, manual_pan);
    let theta = (pan + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
    let pan_l = theta.cos();
    let pan_r = theta.sin();

    let mut track_peak_l = 0.0f32;
    let mut track_peak_r = 0.0f32;

    // Accumulate clips into a temporary mix buffer (reused across callbacks)
    MIX_L.with(|ml| {
    MIX_R.with(|mr| {
    PRE_FADER_L.with(|pfl| {
    PRE_FADER_R.with(|pfr| {
    let mut mix_l = ml.borrow_mut();
    let mut mix_r = mr.borrow_mut();
    mix_l.clear();
    mix_l.resize(frames, 0.0f32);
    mix_r.clear();
    mix_r.resize(frames, 0.0f32);
    let mut pre_l = pfl.borrow_mut();
    let mut pre_r = pfr.borrow_mut();
    pre_l.clear();
    pre_l.resize(frames, 0.0f32);
    pre_r.clear();
    pre_r.resize(frames, 0.0f32);

    // Find the first note-capable effect (instrument)
    let inst_idx = handle.fx_chain.iter().position(|e| e.has_note_input);
    let has_midi_clips = handle.clips.iter().any(|c| !c.midi_notes.is_empty());
    if has_midi_clips && inst_idx.is_none() {
        tracing::warn!(
            track_id = %handle.id,
            "track has MIDI clips but no instrument in FX chain"
        );
    }

    // Dispatch MIDI notes and process any instrument on this track
    if let Some(ii) = inst_idx {
        if let EffectKind::Clap(adapter) = &handle.fx_chain[ii].kind {
            if let Ok(mut a) = adapter.try_lock() {
                MIDI_EVENTS.with(|eb| {
                    let mut buf = eb.borrow_mut();
                    buf.clear();
                    let buf_start = pos as u64;
                    let buf_end = (pos + frames) as u64;
                    for clip in &handle.clips {
                        if clip.midi_notes.is_empty() {
                            continue;
                        }
                        let clip_start = clip.position_frames.load(Ordering::Acquire);
                        let clip_end = clip_start + clip.length_frames.load(Ordering::Acquire);
                        if clip_start >= buf_end || clip_end <= buf_start {
                            continue;
                        }
                        for note in &clip.midi_notes {
                            let note_start = clip_start + note.start_frame;
                            let note_end = note_start + note.duration;

                            if seek_occurred && note_start < buf_start {
                                let pckn = Pckn::new(0u8, 0u8, note.pitch, 0u8);
                                buf.push(&NoteOffEvent::new(0, pckn, 0.0));
                            }

                            if note_end > buf_start && note_start < buf_end {
                                let offset = if note_start < buf_start {
                                    if seek_occurred { 1 } else { 0 }
                                } else {
                                    (note_start - buf_start) as u32
                                };
                                let pckn = Pckn::new(0u8, 0u8, note.pitch, 0u8);
                                let vel = note.velocity as f64 / 127.0;
                                buf.push(&NoteOnEvent::new(offset, pckn, vel));
                            }

                            if note_end >= buf_start && note_end < buf_end {
                                let offset = (note_end - buf_start) as u32;
                                let pckn = Pckn::new(0u8, 0u8, note.pitch, 0u8);
                                buf.push(&NoteOffEvent::new(offset, pckn, 0.0));
                            }
                        }
                    }
                    buf.sort();
                    let n_events = buf.len();
                    if n_events > 0 {
                        tracing::trace!(n_events, "dispatching MIDI events to instrument");
                    }
                    let events = buf.as_input();
                    a.process_with_events(&mut mix_l, &mut mix_r, sample_rate, &events);
                    let has_output = mix_l.iter().any(|&s| s.abs() > 1e-10)
                        || mix_r.iter().any(|&s| s.abs() > 1e-10);
                    if n_events > 0 && !has_output {
                        tracing::warn!("instrument process_with_events produced zero output after {} events", n_events);
                    }
                });
            }
        }
    }

    for clip in &handle.clips {
        if !clip.midi_notes.is_empty() {
            continue;
        }
        let clip_pos = clip.position_frames.load(Ordering::Acquire) as usize;
        let clip_off = clip.offset_frames.load(Ordering::Acquire) as usize;
        let clip_len = clip.length_frames.load(Ordering::Acquire) as usize;
        let clip_gain = f32::from_bits(clip.gain.load(Ordering::Acquire));
        let total = track_vol * clip_gain;
        let f_in = clip.fade_in_frames.load(Ordering::Acquire) as usize;
        let f_out = clip.fade_out_frames.load(Ordering::Acquire) as usize;

        let clip_start = clip_pos;
        let clip_end = clip_pos + clip_len.saturating_sub(clip_off);

        if clip_end <= pos || clip_start >= pos.saturating_add(frames) {
            continue;
        }

        for i in 0..frames {
            let playhead = pos + i;
            if playhead < clip_start || playhead >= clip_end {
                continue;
            }
            let local_frame = (playhead - clip_start) + clip_off;
            let src_idx = local_frame * clip.channels as usize;
            if src_idx >= clip.audio_data.len() {
                break;
            }
            let mono = if clip.channels >= 2 {
                if src_idx + 1 >= clip.audio_data.len() {
                    clip.audio_data[src_idx]
                } else {
                    (clip.audio_data[src_idx] + clip.audio_data[src_idx + 1]) * 0.5
                }
            } else {
                clip.audio_data[src_idx]
            };
            let fade_gain = compute_fade_gain(local_frame, clip_len, f_in, f_out);
            let sample = mono * total * fade_gain;
            mix_l[i] += sample * pan_l;
            mix_r[i] += sample * pan_r;

            // Pre-fader: raw clip sum before track volume, pan, and FX
            let raw = mono * clip_gain * fade_gain;
            pre_l[i] += raw;
            pre_r[i] += raw;
        }
    }

    // Evaluate effect parameter automation lanes
    for lane in &handle.automation_lanes {
        if let Some(eid) = lane.effect_instance_id {
            if let Some(inst) = handle.fx_chain.iter_mut().find(|e| e.id == eid) {
                let val = lane.get_value_at(pos_frames);
                if !val.is_nan() {
                    inst.set_parameter(lane.param_id, val);
                }
            }
        }
    }

    // Apply FX chain: each effect processes the track mix in place
    for (fi, instance) in handle.fx_chain.iter_mut().enumerate() {
        if Some(fi) == inst_idx {
            continue;
        }
        if !instance.is_bypassed() {
            match &mut instance.kind {
                EffectKind::BuiltIn(effect) => {
                    effect.process(&mut mix_l, &mut mix_r, sample_rate);
                }
                EffectKind::Clap(adapter) => {
                    if let Ok(mut a) = adapter.try_lock() {
                        a.process(&mut mix_l, &mut mix_r, sample_rate);
                    }
                }
            }
        }
    }

    // Track peaks
    for i in 0..frames {
        track_peak_l = track_peak_l.max(mix_l[i].abs());
        track_peak_r = track_peak_r.max(mix_r[i].abs());
    }

    handle
        .peak_left
        .store(track_peak_l.to_bits(), Ordering::Release);
    handle
        .peak_right
        .store(track_peak_r.to_bits(), Ordering::Release);
    });
    });
    });
    });
}
