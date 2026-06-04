use crate::project::automation::{AutomationLane, PARAM_PAN, PARAM_VOLUME};

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

/// Evaluates track-level volume and pan automation, returning
/// (track_vol, pan_l, pan_r) ready for mixing.
pub fn evaluate_track_params(
    lanes: &[AutomationLane],
    time_frames: u64,
    manual_vol: f32,
    manual_pan: f32,
) -> (f32, f32, f32) {
    let track_vol = automation_value(lanes, PARAM_VOLUME, time_frames, manual_vol);
    let pan = automation_value(lanes, PARAM_PAN, time_frames, manual_pan);
    let theta = (pan + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
    let pan_l = theta.cos();
    let pan_r = theta.sin();
    (track_vol, pan_l, pan_r)
}

/// Sets effect parameters from automation lanes that reference a specific
/// effect instance.
pub fn evaluate_effect_params(
    lanes: &[AutomationLane],
    fx_chain: &mut [crate::audio::effects::dsp_effect::EffectInstance],
    time_frames: u64,
) {
    for lane in lanes {
        if let Some(eid) = lane.effect_instance_id {
            if let Some(inst) = fx_chain.iter_mut().find(|e| e.id == eid) {
                let val = lane.get_value_at(time_frames);
                if !val.is_nan() {
                    inst.try_set_parameter(lane.param_id, val);
                }
            }
        }
    }
}
