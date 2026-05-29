use crate::audio::mixer::MasterBus;
use crate::audio::process;
use crate::audio::transport::Transport;
use crate::project::track::TrackHandle;
use cpal::traits::{DeviceTrait, HostTrait};
use std::cell::Cell;
use std::cell::RefCell;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

thread_local! {
    pub static SCRATCH_L: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    pub static SCRATCH_R: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    pub static AUDIO_THREAD_NAMED: Cell<bool> = const { Cell::new(false) };
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
        Ok(mut devices) => devices.find(|d| d.name().map_or(false, |n| n == name)),
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

    let stream = device.build_output_stream(
        &cpal::StreamConfig {
            buffer_size,
            sample_rate: default_config.sample_rate(),
            channels,
        },
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            super::engine::audio_callback(data, &transport, &master_bus, &tracks);
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

pub fn mix_tracks(
    track_list: &mut std::sync::MutexGuard<Vec<TrackHandle>>,
    data: &mut [f32],
    pos: usize,
    frames: usize,
    sample_rate: u32,
    master_bus: &Arc<MasterBus>,
) {
    SCRATCH_L.with(|sl| {
        SCRATCH_R.with(|sr| {
            let mut out_l = sl.borrow_mut();
            let mut out_r = sr.borrow_mut();
            out_l.clear();
            out_l.resize(frames, 0.0f32);
            out_r.clear();
            out_r.resize(frames, 0.0f32);

            let any_solo = track_list.iter().any(|h| h.solo.load(Ordering::Acquire));
            for handle in track_list.iter_mut() {
                let muted = handle.mute.load(Ordering::Acquire)
                    || (any_solo && !handle.solo.load(Ordering::Acquire));
                if muted {
                    handle.peak_left.store(0, Ordering::Release);
                    handle.peak_right.store(0, Ordering::Release);
                    continue;
                }
                process::process_track(handle, &mut out_l, &mut out_r, pos, frames, sample_rate);
            }

            master_bus.process(&mut out_l, &mut out_r);

            for i in 0..frames {
                data[i * 2] = out_l[i];
                data[i * 2 + 1] = out_r[i];
            }
        });
    });
}

#[cfg(windows)]
pub fn name_audio_thread() {
    use std::os::windows::ffi::OsStrExt;
    AUDIO_THREAD_NAMED.with(|flag| {
        if flag.get() { return; }
        flag.set(true);
        unsafe {
            type HANDLE = *mut std::ffi::c_void;
            extern "system" {
                fn GetCurrentThread() -> HANDLE;
                fn SetThreadDescription(
                    hThread: HANDLE,
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