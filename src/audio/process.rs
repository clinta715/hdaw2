use crate::project::automation::{AutomationLane, PARAM_PAN, PARAM_VOLUME};
use crate::project::track::TrackHandle;
use std::cell::RefCell;
use std::sync::atomic::Ordering;

thread_local! {
    static MIX_L: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    static MIX_R: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
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
    out_l: &mut [f32],
    out_r: &mut [f32],
    pos: usize,
    frames: usize,
    sample_rate: u32,
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
    let mut mix_l = ml.borrow_mut();
    let mut mix_r = mr.borrow_mut();
    mix_l.clear();
    mix_l.resize(frames, 0.0f32);
    mix_r.clear();
    mix_r.resize(frames, 0.0f32);

    for clip in &handle.clips {
        let clip_pos = clip.position_frames.load(Ordering::Acquire) as usize;
        let clip_off = clip.offset_frames.load(Ordering::Acquire) as usize;
        let clip_len = clip.length_frames.load(Ordering::Acquire) as usize;
        let clip_gain = f32::from_bits(clip.gain.load(Ordering::Acquire));
        let total = track_vol * clip_gain;

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
            let sample = mono * total;
            mix_l[i] += sample * pan_l;
            mix_r[i] += sample * pan_r;
        }
    }

    // Apply FX chain: each effect processes the track mix in place
    for instance in handle.fx_chain.iter_mut() {
        if !instance.is_bypassed() {
            instance.effect.process(&mut mix_l, &mut mix_r, sample_rate);
        }
    }

    // Sum into output and track peaks
    for i in 0..frames {
        out_l[i] += mix_l[i];
        out_r[i] += mix_r[i];
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
}
