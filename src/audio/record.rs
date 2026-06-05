use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;

pub struct RecordingResult {
    pub num_frames: u64,
    pub total_samples: u64,
}

#[allow(clippy::type_complexity)]
pub fn start_recording_thread(
    file_path: PathBuf,
    sample_rate: u32,
    channels: u16,
    buffer_capacity: usize,
) -> Result<
    (
        JoinHandle<Result<RecordingResult, String>>,
        SyncSender<Vec<f32>>,
        Arc<AtomicBool>,
    ),
    String,
> {
    let (tx, rx): (SyncSender<Vec<f32>>, Receiver<Vec<f32>>) = sync_channel(buffer_capacity);
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    let handle = std::thread::Builder::new()
        .name("hdaw-record".into())
        .spawn(move || {
            recording_worker(rx, stop_flag_clone, &file_path, sample_rate, channels)
        })
        .map_err(|e| format!("failed to spawn recording thread: {e}"))?;

    Ok((handle, tx, stop_flag))
}

fn recording_worker(
    rx: Receiver<Vec<f32>>,
    stop_flag: Arc<AtomicBool>,
    file_path: &PathBuf,
    sample_rate: u32,
    channels: u16,
) -> Result<RecordingResult, String> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer =
        hound::WavWriter::create(file_path, spec).map_err(|e| format!("create wav: {e}"))?;

    let mut total_samples = 0u64;

    loop {
        // Check stop flag — drain remaining before exit
        if stop_flag.load(Ordering::Acquire) {
            while let Ok(data) = rx.try_recv() {
                for &sample in &data {
                    writer
                        .write_sample(sample)
                        .map_err(|e| format!("write sample: {e}"))?;
                }
                total_samples += data.len() as u64;
            }
            break;
        }

        match rx.recv() {
            Ok(data) => {
                for &sample in &data {
                    writer
                        .write_sample(sample)
                        .map_err(|e| format!("write sample: {e}"))?;
                }
                total_samples += data.len() as u64;
            }
            Err(_) => {
                // Sender dropped — no more data
                break;
            }
        }
    }

    writer
        .finalize()
        .map_err(|e| format!("finalize wav: {e}"))?;

    Ok(RecordingResult {
        num_frames: total_samples / channels as u64,
        total_samples,
    })
}
