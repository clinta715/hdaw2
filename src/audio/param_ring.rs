use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy)]
pub struct ParamChange {
    pub param_id: u32,
    pub value: f64,
}

pub struct ParamRingBuffer {
    entries: UnsafeCell<Vec<ParamChange>>,
    write_idx: AtomicU64,
    read_idx: AtomicU64,
}

unsafe impl Send for ParamRingBuffer {}
unsafe impl Sync for ParamRingBuffer {}

impl ParamRingBuffer {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.next_power_of_two();
        Self {
            entries: UnsafeCell::new(vec![ParamChange { param_id: 0, value: 0.0 }; cap]),
            write_idx: AtomicU64::new(0),
            read_idx: AtomicU64::new(0),
        }
    }

    fn entries_len(&self) -> usize {
        unsafe { (*self.entries.get()).len() }
    }

    pub fn push(&self, param_id: u32, value: f64) -> bool {
        static OVERFLOW_WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

        let write = self.write_idx.load(Ordering::Acquire);
        let read = self.read_idx.load(Ordering::Acquire);
        let avail = write.wrapping_sub(read);

        if avail >= self.entries_len() as u64 {
            if !OVERFLOW_WARNED.swap(true, Ordering::Relaxed) {
                tracing::warn!("Parameter automation queue overflow — some changes dropped");
            }
            return false;
        }

        let len = self.entries_len();
        let idx = (write % len as u64) as usize;
        let entries = unsafe { &mut *self.entries.get() };
        entries[idx] = ParamChange { param_id, value };
        self.write_idx.store(write.wrapping_add(1), Ordering::Release);
        true
    }

    pub fn drain<'a>(&self, out: &'a mut Vec<ParamChange>, count: usize) -> &'a mut Vec<ParamChange> {
        let write = self.write_idx.load(Ordering::Acquire);
        let read = self.read_idx.load(Ordering::Acquire);
        let avail = write.wrapping_sub(read) as usize;
        let to_drain = avail.min(count);
        let len = self.entries_len();

        out.clear();
        let mut r = read;
        for _ in 0..to_drain {
            let idx = (r % len as u64) as usize;
            let entries = unsafe { &*self.entries.get() };
            out.push(entries[idx]);
            r = r.wrapping_add(1);
        }
        self.read_idx.store(r, Ordering::Release);
        out
    }

    pub fn reset_overflow_warning() {
        static OVERFLOW_WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        OVERFLOW_WARNED.store(false, Ordering::Relaxed);
    }
}