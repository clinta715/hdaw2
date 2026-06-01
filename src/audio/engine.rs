use crate::audio::mixer::MasterBus;
use crate::audio::record::{start_recording_thread, RecordingResult};
use crate::audio::stream;
use crate::audio::transport::Transport;
use crate::project::track::TrackHandle;
use cpal::traits::StreamTrait;
use cpal::BufferSize;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

pub struct RecordingSession {
    pub input_stream: cpal::Stream,
    pub sender: SyncSender<Vec<f32>>,
    pub stop_flag: Arc<std::sync::atomic::AtomicBool>,
    pub worker: JoinHandle<Result<RecordingResult, String>>,
    pub file_path: PathBuf,
    pub start_frame: u64,
    pub channels: u16,
    pub sample_rate: u32,
}

pub struct AudioEngine {
    pub transport: Arc<Transport>,
    pub master_bus: Arc<MasterBus>,
    pub tracks: Arc<Mutex<Vec<TrackHandle>>>,
    needs_rebuild: Arc<std::sync::atomic::AtomicBool>,
    _stream: Option<cpal::Stream>,
    pub recording: Arc<Mutex<Option<RecordingSession>>>,
}

impl AudioEngine {
    pub fn new() -> Self {
        Self {
            transport: Arc::new(Transport::new(44100)),
            master_bus: Arc::new(MasterBus::new()),
            tracks: Arc::new(Mutex::new(Vec::new())),
            needs_rebuild: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            _stream: None,
            recording: Arc::new(Mutex::new(None)),
        }
    }

    pub fn init(&mut self) {
        if let Some(s) = stream::build_stream(
            None, BufferSize::Default,
            &self.transport, &self.master_bus, &self.tracks, &self.needs_rebuild,
        ) {
            if let Err(e) = s.play() {
                tracing::error!("start audio stream: {e}");
                return;
            }
            self._stream = Some(s);
            tracing::info!("audio stream started");
        }
    }

    pub fn check_rebuild(&mut self) {
        if self.needs_rebuild.load(Ordering::Acquire) {
            self.needs_rebuild.store(false, Ordering::Release);
            tracing::info!("rebuilding audio stream after device change");
            self._stream = None;
            self.init();
        }
    }

    pub fn rebuild_stream_with_config(
        &mut self,
        device_name: &str,
        sample_rate: u32,
        buffer_size: BufferSize,
    ) {
        self._stream = None;
        let dev = if device_name.is_empty() { None } else { Some(device_name) };
        if let Some(s) = stream::build_stream(
            dev, buffer_size,
            &self.transport, &self.master_bus, &self.tracks, &self.needs_rebuild,
        ) {
            if sample_rate > 0 {
                self.transport.set_sample_rate(sample_rate);
            }
            if let Err(e) = s.play() {
                tracing::error!("start audio stream: {e}");
                return;
            }
            self._stream = Some(s);
            tracing::info!("audio stream rebuilt with config");
        }
    }

    pub fn available_devices() -> Vec<String> {
        stream::available_devices()
    }

    pub fn add_track(&self, handle: TrackHandle) {
        if let Ok(mut list) = self.tracks.lock() {
            list.push(handle);
        }
    }

    pub fn play(&self) {
        self.transport.play();
    }

    pub fn pause(&self) {
        self.transport.pause();
    }

    pub fn stop(&self) {
        self.transport.stop();
    }

    pub fn start_recording(
        &self,
        file_path: PathBuf,
        sample_rate: u32,
        channels: u16,
        start_frame: u64,
        device_name: Option<&str>,
    ) -> Result<(), String> {
        let (handle, sender, stop_flag) =
            start_recording_thread(file_path.clone(), sample_rate, channels, 64)?;

        let input_stream = stream::build_input_stream(sender.clone(), cpal::BufferSize::Default, device_name)
            .ok_or_else(|| "failed to open input stream".to_string())?;

        if let Err(e) = input_stream.play() {
            stop_flag.store(true, Ordering::Release);
            return Err(format!("start input stream: {e}"));
        }

        let session = RecordingSession {
            input_stream,
            sender,
            stop_flag,
            worker: handle,
            file_path,
            start_frame,
            channels,
            sample_rate,
        };

        if let Ok(mut rec) = self.recording.lock() {
            *rec = Some(session);
        }

        Ok(())
    }

    pub fn stop_recording(&self) -> Option<(RecordingResult, PathBuf, u64)> {
        let session = if let Ok(mut rec) = self.recording.lock() {
            rec.take()
        } else {
            return None;
        };

        let session = session?;
        session.stop_flag.store(true, Ordering::Release);
        // Drop the sender so the worker knows no more data is coming
        drop(session.sender);

        let result = match session.worker.join() {
            Ok(r) => r,
            Err(_) => return None,
        };
        let result = match result {
            Ok(r) => r,
            Err(_) => return None,
        };

        Some((result, session.file_path, session.start_frame))
    }

    pub fn is_recording(&self) -> bool {
        self.recording.lock().ok().map_or(false, |r| r.is_some())
    }
}

pub fn audio_callback(
    data: &mut [f32],
    transport: &Arc<Transport>,
    master_bus: &Arc<MasterBus>,
    tracks: &Arc<Mutex<Vec<TrackHandle>>>,
) {
    stream::name_audio_thread();

    if !transport.is_playing() {
        data.fill(0.0);
        return;
    }
    let Ok(mut track_list) = tracks.try_lock() else {
        data.fill(0.0);
        return;
    };
    if track_list.is_empty() {
        data.fill(0.0);
        return;
    }
    let frames = data.len() / 2;
    let pos = transport.position_frames() as usize;
    let sample_rate = transport.sample_rate();
    let seek_occurred = transport.seek_occurred.swap(false, Ordering::Acquire);

    stream::mix_tracks(&mut track_list, data, pos, frames, sample_rate, master_bus, transport, seek_occurred);
    transport.advance_frames(frames as u64);
    check_loop_wrap(transport);
}

fn check_loop_wrap(transport: &Arc<Transport>) {
    if transport.loop_enabled.load(Ordering::Acquire) {
        let pos = transport.position_frames();
        let (loop_in, loop_out) = transport.load_loop_region();
        if pos >= loop_out && loop_in < loop_out {
            transport.seek_to_frame(loop_in);
        }
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.transport.stop();
    }
}