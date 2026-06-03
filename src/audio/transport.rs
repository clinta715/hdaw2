use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

fn pack_loop_region(loop_in: u64, loop_out: u64) -> u64 {
    (loop_out << 32) | (loop_in & 0xFFFF_FFFF)
}

fn unpack_loop_region(packed: u64) -> (u64, u64) {
    let loop_in = packed & 0xFFFF_FFFF;
    let loop_out = packed >> 32;
    (loop_in, loop_out)
}

pub struct Transport {
    playing: AtomicBool,
    position_frames: AtomicU64,
    sample_rate: AtomicU32,
    pub loop_enabled: AtomicBool,
    loop_region: AtomicU64,
    pub seek_occurred: AtomicBool,
    pub bpm: AtomicU32,
    pub time_signature_num: AtomicU32,
    pub time_signature_den: AtomicU32,
    pub metronome_enabled: AtomicBool,
}

impl Transport {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            playing: AtomicBool::new(false),
            position_frames: AtomicU64::new(0),
            sample_rate: AtomicU32::new(sample_rate),
            loop_enabled: AtomicBool::new(false),
            loop_region: AtomicU64::new(pack_loop_region(0, 0)),
            seek_occurred: AtomicBool::new(false),
            bpm: AtomicU32::new(f32::to_bits(120.0)),
            time_signature_num: AtomicU32::new(4),
            time_signature_den: AtomicU32::new(4),
            metronome_enabled: AtomicBool::new(false),
        }
    }

    pub fn play(&self) {
        self.playing.store(true, Ordering::Release);
    }

    pub fn stop(&self) {
        self.playing.store(false, Ordering::Release);
        self.position_frames.store(0, Ordering::Release);
        self.seek_occurred.store(true, Ordering::Release);
    }

    pub fn pause(&self) {
        self.playing.store(false, Ordering::Release);
    }

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Acquire)
    }

    pub fn position_seconds(&self) -> f64 {
        let frames = self.position_frames.load(Ordering::Acquire);
        frames as f64 / self.sample_rate.load(Ordering::Acquire) as f64
    }

    pub fn position_frames(&self) -> u64 {
        self.position_frames.load(Ordering::Acquire)
    }

    pub fn seek_to_frame(&self, frames: u64) {
        self.position_frames.store(frames, Ordering::Release);
        self.seek_occurred.store(true, Ordering::Release);
    }

    pub fn advance_frames(&self, frames: u64) {
        self.position_frames.fetch_add(frames, Ordering::Release);
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Acquire)
    }

    pub fn set_sample_rate(&self, rate: u32) {
        self.sample_rate.store(rate, Ordering::Release);
    }

    pub fn toggle_loop(&self) {
        let current = self.loop_enabled.load(Ordering::Acquire);
        self.loop_enabled.store(!current, Ordering::Release);
    }

    pub fn set_loop_region(&self, in_frame: u64, out_frame: u64) {
        self.loop_region.store(pack_loop_region(in_frame, out_frame), Ordering::Release);
    }

    pub fn load_loop_region(&self) -> (u64, u64) {
        unpack_loop_region(self.loop_region.load(Ordering::Acquire))
    }

    pub fn store_loop_in(&self, in_frame: u64) {
        let (_, out_frame) = self.load_loop_region();
        self.set_loop_region(in_frame, out_frame);
    }

    pub fn store_loop_out(&self, out_frame: u64) {
        let (in_frame, _) = self.load_loop_region();
        self.set_loop_region(in_frame, out_frame);
    }

    pub fn bpm(&self) -> f64 {
        f64::from(f32::from_bits(self.bpm.load(Ordering::Acquire)))
    }

    pub fn set_bpm(&self, bpm: f64) {
        self.bpm.store(f32::to_bits(bpm as f32), Ordering::Release);
    }

    pub fn time_sig_num(&self) -> u32 {
        self.time_signature_num.load(Ordering::Acquire)
    }

    pub fn set_time_sig_num(&self, n: u32) {
        self.time_signature_num.store(n, Ordering::Release);
    }

    pub fn time_sig_den(&self) -> u32 {
        self.time_signature_den.load(Ordering::Acquire)
    }

    pub fn set_time_sig_den(&self, d: u32) {
        self.time_signature_den.store(d, Ordering::Release);
    }

    pub fn toggle_metronome(&self) {
        let current = self.metronome_enabled.load(Ordering::Acquire);
        self.metronome_enabled.store(!current, Ordering::Release);
    }
}
