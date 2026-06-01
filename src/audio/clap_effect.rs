use crate::audio::clap_host::{HdawClapHost, HdawClapHostShared, make_host_info};
use crate::audio::clap_instance::ClapPluginState;
use crate::audio::effects::parameter::{ParamId, ParameterInfo};
use clack_extensions::note_ports::PluginNotePorts;
use clack_extensions::params::{ParamInfoBuffer, PluginParams};
use clack_host::events::event_types::ParamValueEvent;
use clack_host::events::io::{EventBuffer, InputEvents, OutputEvents};
use clack_host::events::Pckn;
use clack_host::utils::{ClapId, Cookie};
use clack_host::prelude::*;
use clack_host::process::audio_buffers::{
    AudioPortBuffer, AudioPortBufferType, AudioPorts, InputChannel,
};
use std::cell::RefCell;
use std::ffi::CString;
use std::path::Path;
use std::sync::Mutex;

thread_local! {
    static SCRATCH_IN_L: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    static SCRATCH_IN_R: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    static SCRATCH_OUT_L: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    static SCRATCH_OUT_R: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    static COMBINED_EVENTS: RefCell<EventBuffer> = RefCell::new(EventBuffer::with_capacity(256));
}

pub struct ClapEffectAdapter {
    state: ClapPluginState,
    #[allow(dead_code)]
    entry: &'static PluginEntry,
    instance: Option<PluginInstance<HdawClapHost>>,
    audio_processor: Option<PluginAudioProcessor<HdawClapHost>>,
    input_ports: Option<Box<AudioPorts>>,
    output_ports: Option<Box<AudioPorts>>,
    pub has_note_input: bool,
    pending_params: Mutex<Vec<(u32, f64)>>,
}

unsafe impl Send for ClapEffectAdapter {}

impl Drop for ClapEffectAdapter {
    fn drop(&mut self) {
        self.deactivate();
    }
}

impl ClapEffectAdapter {
    pub fn new_instance(plugin_id: &str, path: &Path, sample_rate: u32) -> Result<Self, String> {
        let entry_owned =
            unsafe { PluginEntry::load(path).map_err(|e| format!("Failed to load bundle: {:?}", e))? };
        let entry: &'static PluginEntry = Box::leak(Box::new(entry_owned));
        let host_info = make_host_info().map_err(|e| format!("{:?}", e))?;
        let c_plugin_id = CString::new(plugin_id).map_err(|_| "Invalid plugin ID".to_string())?;

        let mut instance = PluginInstance::<HdawClapHost>::new(
            |_| HdawClapHostShared,
            |_| (),
            entry,
            &c_plugin_id,
            &host_info,
        )
        .map_err(|e| format!("Failed to create instance: {:?}", e))?;

        let params_ext = {
            let shared = instance.plugin_shared_handle();
            shared.get_extension::<PluginParams>()
        };

        let param_infos = if let Some(ref params) = params_ext {
            let mut handle = instance.plugin_handle();
            let count = params.count(&mut handle);
            let mut infos = Vec::with_capacity(count as usize);
            let mut buf = ParamInfoBuffer::new();
            for i in 0..count {
                if let Some(info) = params.get_info(&mut handle, i, &mut buf) {
                    let name = String::from_utf8_lossy(info.name).into_owned();
                    infos.push(ParameterInfo {
                        id: infos.len() as ParamId + 1,
                        name: name.clone(),
                        label: String::new(),
                        min_value: info.min_value as f32,
                        max_value: info.max_value as f32,
                        default_value: info.default_value as f32,
                        flags: crate::audio::effects::parameter::ParameterFlags::default(),
                    });
                }
            }
            infos
        } else {
            Vec::new()
        };

        let note_ports_ext = {
            let shared = instance.plugin_shared_handle();
            shared.get_extension::<PluginNotePorts>()
        };
        let has_note_input = if let Some(ref np) = note_ports_ext {
            let mut handle = instance.plugin_handle();
            np.count(&mut handle, true) > 0
        } else {
            false
        };

        tracing::info!(
            %plugin_id,
            has_note_input,
            num_params = param_infos.len(),
            "CLAP plugin loaded"
        );

        let state = ClapPluginState::new(
            plugin_id.to_string(),
            plugin_id.to_string(),
            path.to_path_buf(),
            param_infos,
        );

        let config = PluginAudioConfiguration {
            sample_rate: sample_rate as f64,
            min_frames_count: 1,
            max_frames_count: 2048,
        };

        let stopped = instance
            .activate(|_, _| (), config)
            .map_err(|e| format!("Failed to activate: {:?}", e))?;

        let processor = PluginAudioProcessor::from(stopped);

        Ok(Self {
            state,
            entry,
            instance: Some(instance),
            audio_processor: Some(processor),
            input_ports: Some(Box::new(AudioPorts::with_capacity(2, 1))),
            output_ports: Some(Box::new(AudioPorts::with_capacity(2, 1))),
            has_note_input,
            pending_params: Mutex::new(Vec::new()),
        })
    }

    pub fn has_note_input(&self) -> bool {
        self.has_note_input
    }

    pub fn name(&self) -> String {
        self.state.name().to_string()
    }

    pub fn parameter_info(&self) -> Vec<ParameterInfo> {
        self.state.parameter_info().to_vec()
    }

    pub fn parameter_value(&self, id: ParamId) -> f32 {
        self.state.parameter_value(id)
    }

    pub fn set_parameter(&mut self, id: ParamId, value: f32) {
        self.state.set_parameter(id, value);
        if let Ok(mut pending) = self.pending_params.lock() {
            pending.push((id as u32, value as f64));
        }
    }

    pub fn is_bypassed(&self) -> bool {
        self.state.is_bypassed()
    }

    pub fn set_bypass(&self, val: bool) {
        self.state.set_bypass(val);
    }

    pub fn deactivate(&mut self) {
        if let Some(instance) = &mut self.instance {
            if let Some(processor) = self.audio_processor.take() {
                if let PluginAudioProcessor::Stopped(stopped) = processor {
                    instance.deactivate(stopped);
                }
            }
        }
    }

    pub fn process(&mut self, input_l: &mut [f32], input_r: &mut [f32], sample_rate: u32) {
        let empty = InputEvents::empty();
        self.process_inner(input_l, input_r, sample_rate, &empty);
    }

    pub fn process_with_events(
        &mut self,
        input_l: &mut [f32],
        input_r: &mut [f32],
        sample_rate: u32,
        events: &InputEvents,
    ) {
        self.process_inner(input_l, input_r, sample_rate, events);
    }

    fn process_inner(
        &mut self,
        input_l: &mut [f32],
        input_r: &mut [f32],
        _sample_rate: u32,
        input_events: &InputEvents,
    ) {
        let processor = match &mut self.audio_processor {
            Some(p) => p,
            None => return,
        };

        let Ok(started) = processor.ensure_processing_started() else {
            tracing::warn!(
                name = self.state.name(),
                "ensure_processing_started failed — plugin will not produce audio"
            );
            return;
        };

        let frames = input_l.len().min(input_r.len());
        let i_ports = match &mut self.input_ports {
            Some(p) => p,
            None => return,
        };
        let o_ports = match &mut self.output_ports {
            Some(p) => p,
            None => return,
        };

        SCRATCH_IN_L.with(|sl| {
        SCRATCH_IN_R.with(|sr| {
        SCRATCH_OUT_L.with(|sol| {
        SCRATCH_OUT_R.with(|sor| {
        let mut in_l_buf = sl.borrow_mut();
        let mut in_r_buf = sr.borrow_mut();
        let mut out_l_buf = sol.borrow_mut();
        let mut out_r_buf = sor.borrow_mut();

        in_l_buf.clear();
        in_l_buf.extend_from_slice(input_l);
        in_r_buf.clear();
        in_r_buf.extend_from_slice(input_r);
        out_l_buf.clear();
        out_l_buf.resize(frames, 0.0);
        out_r_buf.clear();
        out_r_buf.resize(frames, 0.0);

        let audio_inputs = i_ports.with_input_buffers([AudioPortBuffer {
            latency: 0,
            channels: AudioPortBufferType::f32_input_only(
                [in_l_buf.as_mut_slice(), in_r_buf.as_mut_slice()]
                    .into_iter()
                    .map(|b| InputChannel::variable(b)),
            ),
        }]);

        let mut audio_outputs =
            o_ports.with_output_buffers([AudioPortBuffer {
                latency: 0,
                channels: AudioPortBufferType::f32_output_only(
                    [out_l_buf.as_mut_slice(), out_r_buf.as_mut_slice()]
                        .into_iter(),
                ),
            }]);

        let mut output_events = OutputEvents::void();

        COMBINED_EVENTS.with(|ceb| {
            let mut combined = ceb.borrow_mut();
            combined.clear();

            // Copy all input events into combined buffer
            for event in input_events.iter() {
                combined.push(event);
            }

            // Add pending param changes (non-blocking in audio thread)
            if let Ok(mut pending) = self.pending_params.try_lock() {
                let pckn = Pckn::new(0u8, 0u8, 0u8, 0u8);
                for &(id, value) in pending.iter() {
                    let ev = ParamValueEvent::new(
                        0,
                        ClapId::from(id),
                        pckn,
                        value,
                        Cookie::default(),
                    );
                    combined.push(&ev);
                }
                pending.clear();
            }

            combined.sort();

            let events = combined.as_input();
            if let Err(e) = started.process(
                &audio_inputs,
                &mut audio_outputs,
                &events,
                &mut output_events,
                None,
                None,
            ) {
                tracing::warn!("CLAP process error: {:?}", e);
            }
        });
        drop(audio_outputs);
        drop(audio_inputs);

        if self.has_note_input {
            let has_out = out_l_buf.iter().any(|&s| s.abs() > 1e-10)
                || out_r_buf.iter().any(|&s| s.abs() > 1e-10);
            if !has_out {
                tracing::warn!(
                    name = self.state.name(),
                    "process() returned Ok but produced zero output despite being an instrument"
                );
            }
        }

        let written = frames.min(out_l_buf.len());
        input_l[..written].copy_from_slice(&out_l_buf[..written]);
        input_r[..written].copy_from_slice(&out_r_buf[..written]);
        });});});});
    }
}
