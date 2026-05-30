use serde::{Deserialize, Serialize};

pub const PARAM_VOLUME: u32 = 0xFFFF_FFFE;
pub const PARAM_PAN: u32 = 0xFFFF_FFFD;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationPoint {
    pub time_frames: u64,
    pub value: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationLane {
    pub param_id: u32,
    pub param_name: String,
    pub points: Vec<AutomationPoint>,
    #[serde(skip)]
    pub dirty: bool,
}

impl AutomationLane {
    pub fn new(param_id: u32, param_name: String) -> Self {
        Self {
            param_id,
            param_name,
            points: Vec::new(),
            dirty: false,
        }
    }

    pub fn volume_lane() -> Self {
        Self::new(PARAM_VOLUME, "Volume".into())
    }

    pub fn pan_lane() -> Self {
        Self::new(PARAM_PAN, "Pan".into())
    }

    pub fn add_point(&mut self, time_frames: u64, value: f32) {
        self.points.push(AutomationPoint {
            time_frames,
            value,
        });
        self.points.sort_by_key(|p| p.time_frames);
    }

    pub fn get_value_at(&self, time_frames: u64) -> f32 {
        if self.points.is_empty() {
            return f32::NAN;
        }
        if time_frames <= self.points[0].time_frames {
            return self.points[0].value;
        }
        let last = self.points.last().unwrap();
        if time_frames >= last.time_frames {
            return last.value;
        }

        for pair in self.points.windows(2) {
            let a = &pair[0];
            let b = &pair[1];
            if time_frames >= a.time_frames && time_frames <= b.time_frames {
                let t = (time_frames - a.time_frames) as f64
                    / (b.time_frames - a.time_frames) as f64;
                return a.value + (b.value - a.value) * t as f32;
            }
        }
        0.0
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}
