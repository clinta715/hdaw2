use crate::audio::clap_host::{HdawClapHost, HdawClapHostShared, make_host_info};
use crate::audio::clap_instance::ClapPluginState;
use crate::audio::effects::parameter::{ParamId, ParameterInfo};
use crate::audio::param_ring::ParamRingBuffer;
use clack_extensions::gui::{GuiApiType, PluginGui};
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
use std::sync::Arc;

thread_local! {
    static SCRATCH_IN_L: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    static SCRATCH_IN_R: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    static SCRATCH_OUT_L: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    static SCRATCH_OUT_R: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    static COMBINED_EVENTS: RefCell<EventBuffer> = RefCell::new(EventBuffer::with_capacity(256));
    static DRAINED: RefCell<Vec<crate::audio::param_ring::ParamChange>> = RefCell::new(Vec::new());
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
    pub gui_supported: bool,
    gui_created: bool,
    ring_buffer: Arc<ParamRingBuffer>,
}

unsafe impl Send for ClapEffectAdapter {}

impl Drop for ClapEffectAdapter {
    fn drop(&mut self) {
        self.close_gui();
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
                    let param_id = info.id.get();
                infos.push(ParameterInfo {
                    id: param_id,
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

        let gui_ext = {
            let shared = instance.plugin_shared_handle();
            shared.get_extension::<PluginGui>()
        };

        let gui_supported = if let Some(ref gui) = gui_ext {
            let mut handle = instance.plugin_handle();
            gui.is_api_supported(&mut handle, clack_extensions::gui::GuiConfiguration {
                api_type: GuiApiType::WIN32,
                is_floating: false,
            })
        } else {
            false
        };

        tracing::info!(
            %plugin_id,
            has_note_input,
            gui_supported,
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
            max_frames_count: 8192,
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
            gui_supported,
            gui_created: false,
            ring_buffer: Arc::new(ParamRingBuffer::new(1024)),
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
        self.ring_buffer.push(id as u32, value as f64);
    }

    /// Audio-thread method — applies param directly to state without pushing to ring.
    /// Called from drain loop in process_inner while holding the adapter lock.
    pub fn apply_parameter(&self, id: ParamId, value: f32) {
        self.state.set_parameter(id, value);
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

    pub fn open_gui(&mut self, parent: *mut std::ffi::c_void) -> Result<(), String> {
        if self.gui_created {
            return Ok(());
        }
        let instance = self.instance.as_mut().ok_or("No instance")?;
        let gui_ext = instance.plugin_shared_handle().get_extension::<PluginGui>().ok_or("No GUI extension")?;
        let mut handle = instance.plugin_handle();

        let configs = [
            clack_extensions::gui::GuiConfiguration { api_type: GuiApiType::WIN32, is_floating: true },
            clack_extensions::gui::GuiConfiguration { api_type: GuiApiType::WIN32, is_floating: false },
        ];

        let mut chosen = None;
        for &cfg in &configs {
            if gui_ext.is_api_supported(&mut handle, cfg) {
                chosen = Some(cfg);
                break;
            }
        }

        let config = match chosen {
            Some(c) => c,
            None => {
                configs[0]
            }
        };

        gui_ext.create(&mut handle, config)
            .map_err(|e| format!("Failed to create GUI: {:?}", e))?;

        if !config.is_floating {
            unsafe {
                gui_ext.set_parent(&mut handle, clack_extensions::gui::Window::from_win32_hwnd(parent as _))
                    .map_err(|e| format!("Failed to set GUI parent: {:?}", e))?;
            }
        }

        self.gui_created = true;
        Ok(())
    }

    pub fn show_gui(&mut self) -> Result<(), String> {
        let instance = self.instance.as_mut().ok_or("No instance")?;
        let gui_ext = instance.plugin_shared_handle().get_extension::<PluginGui>().ok_or("No GUI extension")?;
        let mut handle = instance.plugin_handle();
        gui_ext.show(&mut handle).map_err(|e| format!("Failed to show GUI: {:?}", e))
    }

    pub fn close_gui(&mut self) {
        if !self.gui_created {
            return;
        }
        if let Some(instance) = &mut self.instance {
            if let Some(gui_ext) = instance.plugin_shared_handle().get_extension::<PluginGui>() {
                let mut handle = instance.plugin_handle();
                let _ = gui_ext.destroy(&mut handle);
            }
        }
        self.gui_created = false;
    }

    pub fn reset(&mut self) {
        if let Some(processor) = &mut self.audio_processor {
            processor.reset();
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
            return;
        };

        let frames = input_l.len().min(input_r.len());
        if frames == 0 { return; }

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
            in_l_buf.resize(frames, 0.0);
            in_l_buf[..frames].copy_from_slice(&input_l[..frames]);
            in_r_buf.clear();
            in_r_buf.resize(frames, 0.0);
            in_r_buf[..frames].copy_from_slice(&input_r[..frames]);
            
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

            let out_slices = [out_l_buf.as_mut_slice(), out_r_buf.as_mut_slice()];
            let mut audio_outputs =
                o_ports.with_output_buffers([AudioPortBuffer {
                    latency: 0,
                    channels: AudioPortBufferType::f32_output_only(
                        out_slices.into_iter()
                    ),
                }]);

            let mut output_events = OutputEvents::void();

            COMBINED_EVENTS.with(|ceb| {
                let mut combined = ceb.borrow_mut();
                combined.clear();

                for event in input_events.iter() {
                    combined.push(event);
                }

                DRAINED.with(|d| {
                    let mut drained = d.borrow_mut();
                    self.ring_buffer.drain(&mut drained, 64);
                    let pckn = Pckn::new(0u8, 0u8, 0u8, 0u8);
                    for change in drained.iter() {
                        let ev = ParamValueEvent::new(
                            0,
                            ClapId::from(change.param_id),
                            pckn,
                            change.value,
                            Cookie::default(),
                        );
                        combined.push(&ev);
                    }
                });

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

            input_l[..frames].copy_from_slice(&out_l_buf[..frames]);
            input_r[..frames].copy_from_slice(&out_r_buf[..frames]);
        });});});});
    }
}
