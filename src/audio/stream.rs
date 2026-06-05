use crate::audio::effects::dsp_effect::EffectKind;
use crate::audio::mixer::MasterBus;
use crate::audio::process;
use crate::audio::transport::Transport;
use crate::project::track::TrackHandle;
use cpal::traits::{DeviceTrait, HostTrait};
use std::cell::Cell;
use std::cell::RefCell;

use std::sync::atomic::Ordering;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};

thread_local! {
    pub static SCRATCH_L: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    pub static SCRATCH_R: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    pub static AUDIO_THREAD_NAMED: Cell<bool> = const { Cell::new(false) };
    static GROUP_ACCUM_L: RefCell<Vec<Vec<f32>>> = const { RefCell::new(Vec::new()) };
    static GROUP_ACCUM_R: RefCell<Vec<Vec<f32>>> = const { RefCell::new(Vec::new()) };
    static RETURN_ACCUM_L: RefCell<Vec<Vec<f32>>> = const { RefCell::new(Vec::new()) };
    static RETURN_ACCUM_R: RefCell<Vec<Vec<f32>>> = const { RefCell::new(Vec::new()) };
    static GROUP_IDXS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static RETURN_IDXS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static GROUP_TO_POS: RefCell<std::collections::HashMap<uuid::Uuid, usize>> = RefCell::new(std::collections::HashMap::new());
    static RETURN_TO_POS: RefCell<std::collections::HashMap<uuid::Uuid, usize>> = RefCell::new(std::collections::HashMap::new());
    static UUID_TO_IDX: RefCell<std::collections::HashMap<uuid::Uuid, usize>> = RefCell::new(std::collections::HashMap::new());
    static IN_DEGREE: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static CHILDREN: RefCell<Vec<Vec<usize>>> = const { RefCell::new(Vec::new()) };
    static KAHN_QUEUE: RefCell<std::collections::VecDeque<usize>> = const { RefCell::new(std::collections::VecDeque::new()) };
    static METRONOME_SIN_TABLE: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
}

pub fn available_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.output_devices() {
        Ok(devices) => devices.filter_map(|d| d.name().ok()).collect(),
        Err(e) => {
            tracing::error!("failed to enumerate audio devices: {e}");
            Vec::new()
        }
    }
}

pub fn find_device(name: &str) -> Option<cpal::Device> {
    let host = cpal::default_host();
    if name.is_empty() {
        return host.default_output_device();
    }
    match host.output_devices() {
        Ok(mut devices) => devices.find(|d| d.name().is_ok_and(|n| n == name)),
        Err(_) => None,
    }
}

pub fn build_stream(
    device_name: Option<&str>,
    buffer_size: cpal::BufferSize,
    transport: &Arc<Transport>,
    master_bus: &Arc<MasterBus>,
    tracks: &Arc<Mutex<Vec<TrackHandle>>>,
    needs_rebuild: &Arc<std::sync::atomic::AtomicBool>,
) -> Option<cpal::Stream> {
    let device = match device_name {
        Some(name) => find_device(name)?,
        None => {
            let host = cpal::default_host();
            host.default_output_device()?
        }
    };
    tracing::info!("audio device: {}", device.name().unwrap_or_default());

    let default_config = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("failed to get output config: {e}");
            return None;
        }
    };

    let sr = default_config.sample_rate().0;
    let channels = default_config.channels();
    transport.set_sample_rate(sr);
    tracing::info!("audio config: {sr} Hz, {channels} channels");

    let transport = transport.clone();
    let master_bus = master_bus.clone();
    let tracks = tracks.clone();
    let rebuild_flag = needs_rebuild.clone();
    let err_flag = rebuild_flag.clone();

    let stream = device.build_output_stream(
        &cpal::StreamConfig {
            buffer_size,
            sample_rate: default_config.sample_rate(),
            channels,
        },
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                super::engine::audio_callback(data, channels, &transport, &master_bus, &tracks);
            }));
            if result.is_err() {
                data.fill(0.0);
                // Recover any mutex poisoned by the panic — guards were dropped during unwind
                tracks.lock().ok();
                err_flag.store(true, Ordering::Release);
            }
        },
        move |_err| {
            rebuild_flag.store(true, Ordering::Release);
        },
        None,
    );

    match stream {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::error!("build audio stream: {e}");
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn mix_tracks(
    track_list: &mut std::sync::MutexGuard<Vec<TrackHandle>>,
    data: &mut [f32],
    channels: u16,
    pos: usize,
    frames: usize,
    sample_rate: u32,
    master_bus: &Arc<MasterBus>,
    transport: &Arc<Transport>,
    seek_occurred: bool,
) {
    let n_tracks = track_list.len();
    let any_solo = track_list.iter().any(|h| h.solo.load(Ordering::Acquire));

    SCRATCH_L.with(|sl| {
    SCRATCH_R.with(|sr| {
    GROUP_ACCUM_L.with(|gl| {
    GROUP_ACCUM_R.with(|gr| {
    RETURN_ACCUM_L.with(|rl| {
    RETURN_ACCUM_R.with(|rr| {
    GROUP_IDXS.with(|gi| {
    RETURN_IDXS.with(|ri| {
    GROUP_TO_POS.with(|gtp| {
    RETURN_TO_POS.with(|rtp| {
    UUID_TO_IDX.with(|ui| {
    let mut out_l = sl.borrow_mut();
    let mut out_r = sr.borrow_mut();
    out_l.clear();
    out_l.resize(frames, 0.0f32);
    out_r.clear();
    out_r.resize(frames, 0.0f32);

    let mut group_indices = gi.borrow_mut();
    let mut return_indices = ri.borrow_mut();
    let mut uuid_to_idx = ui.borrow_mut();
    group_indices.clear();
    return_indices.clear();
    uuid_to_idx.clear();
    for i in 0..n_tracks {
        if track_list[i].is_group {
            group_indices.push(i);
        } else if track_list[i].is_return {
            return_indices.push(i);
        }
        uuid_to_idx.insert(track_list[i].id, i);
    }
    let n_groups = group_indices.len();
    let n_returns = return_indices.len();

    let mut group_to_pos = gtp.borrow_mut();
    group_to_pos.clear();
    for (pos, &ti) in group_indices.iter().enumerate() {
        group_to_pos.insert(track_list[ti].id, pos);
    }
    drop(group_to_pos);

    let mut return_to_pos = rtp.borrow_mut();
    return_to_pos.clear();
    for (pos, &ti) in return_indices.iter().enumerate() {
        return_to_pos.insert(track_list[ti].id, pos);
    }
    drop(return_to_pos);

    // Resize group accumulators
    let mut g_l = gl.borrow_mut();
    let mut g_r = gr.borrow_mut();
    g_l.resize(n_groups, Vec::new());
    g_r.resize(n_groups, Vec::new());
    for gi in 0..n_groups {
        g_l[gi].clear();
        g_l[gi].resize(frames, 0.0f32);
        g_r[gi].clear();
        g_r[gi].resize(frames, 0.0f32);
    }

    // Resize return accumulators
    let mut r_l = rl.borrow_mut();
    let mut r_r = rr.borrow_mut();
    r_l.resize(n_returns, Vec::new());
    r_r.resize(n_returns, Vec::new());
    for ri in 0..n_returns {
        r_l[ri].clear();
        r_l[ri].resize(frames, 0.0f32);
        r_r[ri].clear();
        r_r[ri].resize(frames, 0.0f32);
    }
    drop(g_l);
    drop(g_r);
    drop(r_l);
    drop(r_r);

    // ===== Pass 1: Source tracks (non-group, non-return) =====
    for i in 0..n_tracks {
        let muted = {
            let h = &track_list[i];
            h.mute.load(Ordering::Acquire) || (any_solo && !h.solo.load(Ordering::Acquire))
        };
        if muted || track_list[i].is_group || track_list[i].is_return {
            if muted {
                track_list[i].peak_left.store(0, Ordering::Release);
                track_list[i].peak_right.store(0, Ordering::Release);
            }
            continue;
        }

        process::process_track(&mut track_list[i], pos, frames, sample_rate, seek_occurred);

        // Read post-fader + pre-fader from thread-locals
        process::MIX_L.with(|ml| {
        process::MIX_R.with(|mr| {
        process::PRE_FADER_L.with(|pfl| {
        process::PRE_FADER_R.with(|pfr| {
            let mix_l = ml.borrow();
            let mix_r = mr.borrow();
            let pre_l = pfl.borrow();
            let pre_r = pfr.borrow();

            // Route post-fader signal
            let pg = track_list[i].parent_group;
            if let Some(pid) = pg {
                if let Some(&pos) = gtp.borrow().get(&pid) {
                        let mut g_l = gl.borrow_mut();
                        let mut g_r = gr.borrow_mut();
                        for s in 0..frames {
                            g_l[pos][s] += mix_l[s];
                            g_r[pos][s] += mix_r[s];
                        }
                    }
            } else {
                for s in 0..frames {
                    out_l[s] += mix_l[s];
                    out_r[s] += mix_r[s];
                }
            }

            // Route sends
            for send in &track_list[i].sends {
                let send_level = f32::from_bits(send.level.load(Ordering::Acquire));
                if send_level < 0.001 { continue; }
                if let Some(&pos) = rtp.borrow().get(&send.target_id) {
                    let (src_l, src_r) = if send.pre_fader {
                        (&*pre_l, &*pre_r)
                    } else {
                        (&*mix_l, &*mix_r)
                    };
                    let mut r_l = rl.borrow_mut();
                    let mut r_r = rr.borrow_mut();
                    for s in 0..frames {
                        r_l[pos][s] += src_l[s] * send_level;
                        r_r[pos][s] += src_r[s] * send_level;
                    }
                }
            }
        });});});
        });
    }

    // ===== Pass 2: Group tracks (topological order) =====
    if n_groups > 0 {
        IN_DEGREE.with(|id| {
        CHILDREN.with(|ch| {
        KAHN_QUEUE.with(|kq| {
        let mut in_degree = id.borrow_mut();
        let mut children = ch.borrow_mut();
        let mut queue = kq.borrow_mut();
        in_degree.clear();
        in_degree.resize(n_groups, 0usize);
        children.clear();
        children.resize(n_groups, Vec::new());
        for gi in 0..n_groups {
            let g_idx = group_indices[gi];
            let pg = track_list[g_idx].parent_group;
            if let Some(pid) = pg {
                if let Some(&parent_gi) = gtp.borrow().get(&pid) {
                    in_degree[parent_gi] += 1;
                    children[gi].push(parent_gi);
                }
            }
        }
        queue.clear();
        for gi in 0..n_groups {
            if in_degree[gi] == 0 {
                queue.push_back(gi);
            }
        }
        while !queue.is_empty() {
            let gi = queue.pop_front().unwrap();
            let g_idx = group_indices[gi];
            let muted = {
                let h = &track_list[g_idx];
                h.mute.load(Ordering::Acquire) || (any_solo && !h.solo.load(Ordering::Acquire))
            };

            if !muted {
                for instance in track_list[g_idx].fx_chain.iter_mut() {
                    if !instance.is_bypassed() {
                        match &mut instance.kind {
                            EffectKind::BuiltIn(effect) => {
                                let mut g_l = gl.borrow_mut();
                                let mut g_r = gr.borrow_mut();
                                effect.process(&mut g_l[gi], &mut g_r[gi], sample_rate);
                            }
                            EffectKind::Clap(adapter) => {
                                if let Ok(mut a) = adapter.try_lock() {
                                    let mut g_l = gl.borrow_mut();
                                    let mut g_r = gr.borrow_mut();
                                    a.process(&mut g_l[gi], &mut g_r[gi], sample_rate);
                                } else {
                                    #[allow(clippy::mut_mutex_lock)]
                                    adapter.lock().ok();
                                }
                            }
                        }
                    }
                }
            }

            // Route group output to parent or master
            let pg = track_list[g_idx].parent_group;
            let mut g_l = gl.borrow_mut();
            let mut g_r = gr.borrow_mut();
            if let Some(pid) = pg {
                if let Some(&parent_gi) = gtp.borrow().get(&pid) {
                    for s in 0..frames {
                        g_l[parent_gi][s] += g_l[gi][s];
                        g_r[parent_gi][s] += g_r[gi][s];
                    }
                }
            } else {
                for s in 0..frames {
                    out_l[s] += g_l[gi][s];
                    out_r[s] += g_r[gi][s];
                }
            }
            drop(g_l);
            drop(g_r);

            // Group sends
            for send in &track_list[g_idx].sends {
                let send_level = f32::from_bits(send.level.load(Ordering::Acquire));
                if send_level < 0.001 { continue; }
                if let Some(&pos) = rtp.borrow().get(&send.target_id) {
                    let mut r_l = rl.borrow_mut();
                    let mut r_r = rr.borrow_mut();
                    let g_l = gl.borrow();
                    let g_r = gr.borrow();
                    for s in 0..frames {
                        r_l[pos][s] += g_l[gi][s] * send_level;
                        r_r[pos][s] += g_r[gi][s] * send_level;
                    }
                }
            }

            // Decrement parents' in_degree
            for &parent_gi in &children[gi] {
                in_degree[parent_gi] = in_degree[parent_gi].saturating_sub(1);
                if in_degree[parent_gi] == 0 && !queue.contains(&parent_gi) {
                    queue.push_back(parent_gi);
                }
            }
        }
        });});});
    }

    // ===== Pass 3: Return tracks =====
    for ri in 0..n_returns {
        let r_idx = return_indices[ri];
        let muted = {
            let h = &track_list[r_idx];
            h.mute.load(Ordering::Acquire) || (any_solo && !h.solo.load(Ordering::Acquire))
        };
        if muted { continue; }

        // Process FX chain on return accumulator
        for instance in track_list[r_idx].fx_chain.iter_mut() {
            if !instance.is_bypassed() {
                match &mut instance.kind {
                    EffectKind::BuiltIn(effect) => {
                        let mut r_l = rl.borrow_mut();
                        let mut r_r = rr.borrow_mut();
                        effect.process(&mut r_l[ri], &mut r_r[ri], sample_rate);
                    }
                    EffectKind::Clap(adapter) => {
                        if let Ok(mut a) = adapter.try_lock() {
                            let mut r_l = rl.borrow_mut();
                            let mut r_r = rr.borrow_mut();
                            a.process(&mut r_l[ri], &mut r_r[ri], sample_rate);
                        } else {
                            #[allow(clippy::mut_mutex_lock)]
                            adapter.lock().ok();
                        }
                    }
                }
            }
        }

        // Route to master
        let r_l = rl.borrow();
        let r_r = rr.borrow();
        for s in 0..frames {
            out_l[s] += r_l[ri][s];
            out_r[s] += r_r[ri][s];
        }
    }

    // ===== Pass 4: Master bus + metronome =====
    master_bus.process(&mut out_l, &mut out_r);

    if transport.metronome_enabled.load(Ordering::Acquire) {
        let bpm = transport.bpm();
        let beats_per_sec = bpm / 60.0;
        let sr = sample_rate as f64;
        let start_beat = pos as f64 * beats_per_sec / sr;
        let end_beat = (pos + frames) as f64 * beats_per_sec / sr;
        let first_beat = (start_beat + 1.0).floor();
        let mut beat = first_beat;
        while beat <= end_beat {
            let beat_sample = ((beat * sr / beats_per_sec) - pos as f64) as usize;
            if beat_sample < frames {
                let click_len = (0.01 * sr).min((frames - beat_sample) as f64) as usize;
                let click_freq = 1000.0 / sr;
                METRONOME_SIN_TABLE.with(|tbl| {
                    let mut table = tbl.borrow_mut();
                    let needed = click_len;
                    if table.len() < needed {
                        table.resize(needed, 0.0);
                        for i in 0..needed {
                            table[i] = (i as f64 * click_freq * std::f64::consts::TAU).sin();
                        }
                    }
                    for j in 0..needed {
                        let idx = beat_sample + j;
                        if idx >= frames { break; }
                        let env = 1.0 - (j as f64 / click_len as f64);
                        let sample = table[j] as f32 * 0.3 * env as f32;
                        out_l[idx] += sample;
                        out_r[idx] += sample;
                    }
                });
            }
            beat += 1.0;
        }
    }

    for i in 0..frames {
        for c in 0..channels as usize {
            let idx = i * channels as usize + c;
            if idx < data.len() {
                if c == 0 {
                    data[idx] = out_l[i];
                } else if c == 1 {
                    data[idx] = out_r[i];
                } else {
                    data[idx] = 0.0;
                }
            }
        }
    }
    });});});});});});});});});});});
}

#[cfg(windows)]
pub fn name_audio_thread() {
    use std::os::windows::ffi::OsStrExt;
    AUDIO_THREAD_NAMED.with(|flag| {
        if flag.get() { return; }
        flag.set(true);
        unsafe {
            type Handle = *mut std::ffi::c_void;
            extern "system" {
                fn GetCurrentThread() -> Handle;
                fn SetThreadDescription(
                    hThread: Handle,
                    lpThreadDescription: *const u16,
                ) -> std::ffi::c_long;
            }
            let name: Vec<u16> = std::ffi::OsStr::new("hdaw-audio")
                .encode_wide()
                .chain(Some(0))
                .collect();
            SetThreadDescription(GetCurrentThread(), name.as_ptr());
        }
    });
}

#[cfg(not(windows))]
pub fn name_audio_thread() {}

/// Offline render for export. Processes tracks in the given frame range without
/// touching the real-time Transport (no advance_frames, no loop, no metronome).
/// Returns interleaved stereo f32 samples.
pub fn render_export(
    tracks: &mut [TrackHandle],
    master_bus: &MasterBus,
    sample_rate: u32,
    start_frame: u64,
    end_frame: u64,
) -> Vec<f32> {
    let total_frames = (end_frame - start_frame) as usize;
    let mut output = Vec::with_capacity(total_frames * 2);

    const CHUNK: usize = 1024;

    SCRATCH_L.with(|sl| {
        SCRATCH_R.with(|sr| {
            let mut out_l = sl.borrow_mut();
            let mut out_r = sr.borrow_mut();

            let mut pos = start_frame as usize;
            while pos < end_frame as usize {
                let frames = CHUNK.min((end_frame as usize).saturating_sub(pos));

                out_l.clear();
                out_l.resize(frames, 0.0f32);
                out_r.clear();
                out_r.resize(frames, 0.0f32);

                let any_solo = tracks.iter().any(|h| h.solo.load(Ordering::Acquire));
                for handle in tracks.iter_mut() {
                    let muted = handle.mute.load(Ordering::Acquire)
                        || (any_solo && !handle.solo.load(Ordering::Acquire));
                    if muted {
                        continue;
                    }
                    process::process_track(handle, pos, frames, sample_rate, false);
                    process::MIX_L.with(|ml| {
                        process::MIX_R.with(|mr| {
                            let mix_l = ml.borrow();
                            let mix_r = mr.borrow();
                            for i in 0..frames.min(mix_l.len()) {
                                out_l[i] += mix_l[i];
                                out_r[i] += mix_r[i];
                            }
                        });
                    });
                }

                master_bus.process(&mut out_l, &mut out_r);

                for i in 0..frames {
                    output.push(out_l[i]);
                    output.push(out_r[i]);
                }

                pos += frames;
            }
        });
    });

    output
}

pub fn build_input_stream(
    sender: SyncSender<Vec<f32>>,
    buffer_size: cpal::BufferSize,
    device_name: Option<&str>,
) -> Option<cpal::Stream> {
    let host = cpal::default_host();
    let device = match device_name {
        Some(name) => {
            match host.input_devices() {
                Ok(mut devices) => devices.find(|d| d.name().is_ok_and(|n| n == name)),
                Err(_) => None,
            }
        }
        None => host.default_input_device(),
    };
    let device = device?;

    let config = match device.default_input_config() {
        Ok(c) => cpal::StreamConfig {
            buffer_size,
            sample_rate: cpal::SampleRate(c.sample_rate().0),
            channels: c.channels(),
        },
        Err(_) => return None,
    };

    let stream = device
        .build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let _ = sender.try_send(data.to_vec());
            },
            move |err| {
                tracing::error!("input stream error: {err}");
            },
            None,
        )
        .ok()?;

    Some(stream)
}