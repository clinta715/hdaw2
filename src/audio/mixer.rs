use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub struct MasterBus {
    volume: Arc<AtomicU32>,
    pub peak_left: Arc<AtomicU32>,
    pub peak_right: Arc<AtomicU32>,
}

impl MasterBus {
    pub fn new() -> Self {
        Self {
            volume: Arc::new(AtomicU32::new(f32::to_bits(1.0))),
            peak_left: Arc::new(AtomicU32::new(0)),
            peak_right: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn volume(&self) -> Arc<AtomicU32> {
        self.volume.clone()
    }

    pub fn set_volume(&self, vol: f32) {
        self.volume.store(vol.to_bits(), Ordering::Release);
    }

    pub fn get_volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Acquire))
    }

    pub fn process(&self, left: &mut [f32], right: &mut [f32]) {
        let vol = f32::from_bits(self.volume.load(Ordering::Acquire));
        let mut peak_l = 0.0f32;
        let mut peak_r = 0.0f32;

        for i in 0..left.len() {
            let mut l = left[i];
            let mut r = right[i];
            
            // NAN/Inf protection
            if !l.is_finite() { l = 0.0; }
            if !r.is_finite() { r = 0.0; }

            l *= vol;
            r *= vol;
            
            left[i] = l;
            right[i] = r;

            peak_l = peak_l.max(l.abs());
            peak_r = peak_r.max(r.abs());
        }

        self.peak_left.store(peak_l.to_bits(), Ordering::Release);
        self.peak_right.store(peak_r.to_bits(), Ordering::Release);
    }
}

impl Default for MasterBus {
    fn default() -> Self {
        Self::new()
    }
}
