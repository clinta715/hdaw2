use crate::audio::automation_proc;
use crate::audio::effects::dsp_effect::EffectKind;
use crate::audio::midi_dispatch;
use crate::project::track::TrackHandle;
use std::cell::RefCell;
use std::sync::atomic::Ordering;

thread_local! {
    pub static MIX_L: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    pub static MIX_R: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    pub static PRE_FADER_L: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    pub static PRE_FADER_R: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
}

/// Evaluate clip gain envelope at a given local frame.
/// Linearly interpolates between gain points. Returns 1.0 if no points.
fn eval_gain_envelope(local_frame: u64, points: &[crate::project::clip::GainPoint]) -> f32 {
    if points.is_empty() {
        return 1.0;
    }
    if local_frame <= points[0].time_frames {
        return points[0].value;
    }
    if local_frame >= points.last().unwrap().time_frames {
        return points.last().unwrap().value;
    }
    for pair in points.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        if local_frame >= a.time_frames && local_frame <= b.time_frames {
            let t = (local_frame - a.time_frames) as f64 / (b.time_frames - a.time_frames) as f64;
            return a.value + (b.value - a.value) * t as f32;
        }
    }
    1.0
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

/// Orchestrates per-track processing: automation evaluation, MIDI dispatch,
/// audio clip mixing, effect-parameter automation, and FX chain application.
pub fn process_track(
    handle: &mut TrackHandle,
    pos: usize,
    frames: usize,
    sample_rate: u32,
    seek_occurred: bool,
) {
    // 1. Evaluate track-level automation (volume, pan)
    let manual_vol = f32::from_bits(handle.volume.load(Ordering::Acquire));
    let manual_pan = f32::from_bits(handle.pan.load(Ordering::Acquire));
    let (track_vol, pan_l, pan_r) = automation_proc::evaluate_track_params(
        &handle.automation_lanes,
        pos as u64,
        manual_vol,
        manual_pan,
    );

    let mut track_peak_l = 0.0f32;
    let mut track_peak_r = 0.0f32;

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

    // 2. Dispatch MIDI to instrument (writes into mix_l/mix_r)
    let inst_idx = midi_dispatch::dispatch_midi(
        handle, pos, frames, sample_rate, seek_occurred,
        &mut mix_l, &mut mix_r, track_vol,
    );

    // 3. Mix audio clips into the same buffers
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

        let overlap_start = pos.max(clip_start);
        let overlap_end = (pos + frames).min(clip_end);
        let overlap_len = overlap_end.saturating_sub(overlap_start);
        if overlap_len == 0 {
            continue;
        }

        let buf_offset = overlap_start - pos;
        let local_start = (overlap_start - clip_start) + clip_off;

        let channels = clip.channels as usize;
        let data_len = clip.audio_data.len();

        for i in 0..overlap_len {
            let local_frame = local_start + i;
            let src_idx = local_frame * channels;
            let buf_idx = buf_offset + i;

            if src_idx >= data_len {
                break;
            }

            let (l, r) = if channels >= 2 {
                if src_idx + 1 >= data_len {
                    (clip.audio_data[src_idx] * 0.5, clip.audio_data[src_idx] * 0.5)
                } else {
                    (clip.audio_data[src_idx] * 0.5, clip.audio_data[src_idx + 1] * 0.5)
                }
            } else {
                (clip.audio_data[src_idx], clip.audio_data[src_idx])
            };

            let fade_gain = compute_fade_gain(local_frame, clip_len, f_in, f_out);
            let env_gain = eval_gain_envelope(local_frame as u64, &clip.gain_points);
            let fade_sample = fade_gain * total * env_gain;
            mix_l[buf_idx] += l * pan_l * fade_sample;
            mix_r[buf_idx] += r * pan_r * fade_sample;

            let raw_fade = fade_gain * clip_gain;
            pre_l[buf_idx] += l * 2.0 * raw_fade;
            pre_r[buf_idx] += r * 2.0 * raw_fade;
        }
    }

    // 4. Evaluate effect-parameter automation
    automation_proc::evaluate_effect_params(
        &handle.automation_lanes,
        &mut handle.fx_chain,
        pos as u64,
    );

    // 5. Process FX chain (skip instrument slot)
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
                    } else {
                        adapter.lock().ok();
                    }
                }
            }
        }
    }

    // 6. Compute track peaks
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
