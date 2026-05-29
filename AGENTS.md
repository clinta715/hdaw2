# HDAW Architecture Guide for AI Agents

## Core Architecture

### Dual-Model Sync (Critical Architectural Debt)
Two parallel data models must be manually kept in sync:
- **Project model** (`Project`, `Track`, `AudioClip`, `AutomationLane`) — serializable, used for save/load and UI display
- **Engine model** (`TrackHandle`, `ClipHandle`, `EffectInstance`) — real-time, uses atomics, used by audio callback

**Every operation that modifies a clip/track/effect must update BOTH models.** The sync is ad-hoc:
- `app/commands.rs` — `update_clip_position`, `update_clip_trim`, `remove_selected_clip` touch both
- `app/project_io.rs` — `snapshot_fx_to_project` reads engine → project for serialization
- `ui/timeline/auto_interaction.rs` — `sync_automation_to_project` copies engine auto points → project model
- `ui/effect_editor/` — reads FX chain state from engine directly (not from project model)

### Real-Time Audio Thread Rules
- **NO heap allocations in audio callback.** Use `thread_local!` scratch buffers (`stream.rs`, `process.rs`) that `resize()` instead of allocating new `Vec`.
- **NO locks in audio callback.** Use `try_lock()` only. The UI uses blocking `lock()`.
- Thread naming: `hdaw-audio` on Windows via `SetThreadDescription` (one-time in callback via `thread_local! Cell<bool>`).
- ~19 `lock()` calls from UI code into the engine `Arc<Mutex<Vec<TrackHandle>>>` — lock contention risk.

### Audio Pipeline
`audio_callback()` → `stream::name_audio_thread()` → `stream::mix_tracks()` (per-track: automation → clips → FX chain → mix sum) → `master_bus.process()` → interleave to output

### Atomics for Real-Time Safety
| Type | Use | Access |
|------|-----|--------|
| `AtomicU32` | f32 params (volume, pan, gain) | `f32::to_bits`/`from_bits` |
| `AtomicBool` | mute, solo, bypass | `Ordering::Acquire`/`Release` |
| `AtomicU64` | position frames, packed loop region | audio reads, UI writes |

### CLAP Plugin Hosting Architecture
- **Scanner** (`audio/clap_scanner.rs`): Discovers `.clap` plugin files in OS-standard directories, loads entry points via `clack_host::PluginEntry::load()`, extracts `PluginDescriptor` metadata (name, id, features, is_instrument)
- **Host** (`audio/clap_host.rs`): Implements `HostHandlers` for `clack_host`, provides `HdawClapHost` with logging via `tracing`
- **Plugin State** (`audio/clap_instance.rs`): `ClapPluginState` holds plugin metadata, parameter info/values, bypass state. Parameters bridged to HDAW's `AtomicU32` pattern for lock-free audio thread access
- **Effect Adapter** (`audio/clap_effect.rs`): `ClapEffectAdapter` wraps a `ClapPluginState` behind `Mutex` for thread safety. Currently a pass-through placeholder for audio processing (N1.8 will implement actual CLAP `process()` calls)
- **EffectKind enum** in `dsp_effect.rs`: `EffectInstance.kind` is either `BuiltIn(Box<dyn DspEffect>)` or `Clap(Mutex<ClapEffectAdapter>)`. All `EffectInstance` methods (`parameter_info`, `parameter_value`, `set_parameter`, `is_bypassed`) dispatch based on variant
- **Transport**: Play/Pause/Stop — `pause()` preserves position, `stop()` resets to zero. `Space` = play/pause toggle
- **EffectType** has a `Clap { plugin_id, path }` variant for serialized CLAP plugin references

### Transport Architecture
- `Transport.playing: AtomicBool` — play/pause/stop via `play()`, `pause()`, `stop()`
- **Play** sets `playing=true`
- **Pause** sets `playing=false` (preserves position)
- **Stop** sets `playing=false` AND resets position to 0
- `loop_region: AtomicU64` — packed as (loop_out << 32) | loop_in to avoid torn reads
- UI triggers via `play_requested`, `pause_requested`, `stop_requested` flags

### Automation Architecture
- `AutomationLane` in both project and engine models
- Volume + Pan lanes auto-created per track (sentinel IDs `PARAM_VOLUME`, `PARAM_PAN`)
- `process_track()` evaluates automation per buffer using `get_value_at(pos)`
- f32::NAN from empty lane = use manual atomic value
- Local-override: automation curves don't overwrite atomics; result used locally per buffer
- Deferred sync: engine edits copied to project model every frame (diff check in `sync_automation_to_project`)

### Effect Parameter Pattern
- `DspEffect` trait: `process(&mut self)` (mutable DSP) + `Parameterizable` (immutable reads via `ParameterValue`)
- `ParameterValue` wraps `AtomicU32` for lock-free audio thread reads
- Parameter changes: UI calls `effect.set_parameter()` → atomic store → effect reads on next `process()` call
- Dirty flag pattern on EQ: marks coeffs need rebuild, rebuilds on next `process()` call

### Thread-Local Scratch Buffers
```
stream.rs: SCRATCH_L, SCRATCH_R  — output accumulation
process.rs: MIX_L, MIX_R         — per-track clip mixing
```
These are `thread_local! RefCell<Vec<f32>>` that grow on first use but stabilize capacity after a few callbacks.

### Preferences System
- `PreferencesState` in `ui/preferences.rs` — audio config, project defaults, UI layout values
- Persisted via RON to `%APPDATA%/hdaw/preferences.ron` (or `$HOME/hdaw/` on non-Windows)
- Loaded at startup, saved on Apply or file dialog directory changes
- **Timeline layout** (`header_width`, `track_height`) stored in `TimelineState`, initialized from preferences
- Applied via `apply_preferences()` — rebuilds audio stream + updates timeline layout values
- File dialog directories (`last_import_dir`, `last_open_dir`, `last_save_dir`) persisted alongside

### Timeline Layout (Dynamic, Not Constants)
- `header_width` and `track_height` are **NOT** compile-time constants — they live in `TimelineState`
- All timeline submodules receive these as `f32` parameters from the parent `render()` call
- `DEFAULT_HEADER_WIDTH` (220.0) and `DEFAULT_TRACK_HEIGHT` (80.0) are fallback defaults only
- Changing these in preferences → Apply immediately updates all timeline rendering

## File Map (Current)

| File | Lines | Purpose |
|------|-------|---------|
| `app/mod.rs` | 212 | HdawApp struct, constructor, accessors, `eframe::App::update` |
| `app/commands.rs` | 203 | Clip/track manipulation ops, pool clip restore |
| `app/project_io.rs` | 207 | Save/load/new/import, sync_engine_to_project |
| `app/input.rs` | 158 | Keyboard handling, pending requests, file dialogs |
| `app/prefs_io.rs` | 34 | Preferences load/save via RON |
| `app/undo/mod.rs` | 95 | UndoCommand enum, UndoStack |
| `app/undo/commands.rs` | 219 | apply_undo/apply_redo implementations |
| `ui/timeline/mod.rs` | 223 | Timeline render, zoom/scroll, grid, track layout |
| `ui/timeline/clips.rs` | 172 | Clip waveform drawing + drag/trim interaction |
| `ui/timeline/automation.rs` | 171 | Automation curve drawing + point editing helpers |
| `ui/timeline/interaction.rs` | 172 | Seek, loop, clip, track header interaction handlers |
| `ui/timeline/auto_interaction.rs` | 133 | Automation point interaction + sync to project |
| `ui/timeline/ruler.rs` | 121 | Time ruler ticks, labels, loop markers |
| `ui/timeline/track_headers.rs` | 139 | Track header drawing, mute/solo buttons, hit testing |
| `ui/timeline/playhead.rs` | 14 | Playhead line drawing |
| `ui/effect_editor/mod.rs` | 225 | FX chain panel + effect parameter UI |
| `ui/effect_editor/eq_graph.rs` | 82 | EQ frequency response graph |
| `ui/preferences.rs` | 206 | Preferences dialog (3 sections: Audio, Project, Timeline/UI) |
| `ui/toolbar.rs` | 169 | Top toolbar with transport controls, menus |
| `ui/app_ui.rs` | 136 | Main layout: toolbar, panels, timeline, status bar |
| `ui/mixer_panel.rs` | 94 | Mixer strip panel (reads TrackUiState atomics directly) |
| `ui/audio_pool.rs` | 81 | Audio pool panel for imported files |
| `audio/engine.rs` | 132 | AudioEngine struct, init, play/pause/stop, rebuild |
| `audio/stream.rs` | 147 | build_stream, mix_tracks, name_audio_thread, scratch buffers |
| `audio/process.rs` | 103 | Per-track audio processing (automation → clips → FX) |
| `audio/transport.rs` | 77 | Transport: play/pause/stop, packed loop region, position |
| `audio/clap_scanner.rs` | 100 | CLAP plugin discovery in OS-standard directories |
| `audio/clap_host.rs` | 47 | HDAW CLAP host handlers (logging, lifecycle) |
| `audio/clap_instance.rs` | 75 | ClapPluginState: plugin metadata, parameter bridge |
| `audio/clap_effect.rs` | 44 | ClapEffectAdapter: Mutex-wrapped plugin processing stub |
| `audio/mixer.rs` | 43 | Master bus volume + peak metering |
| `audio/effects/` | ~540 | 5 effects (Gain, EQ, Delay, Reverb, Compressor) + traits + factory |
| `dsp/biquad.rs` | 89 | Shared biquad filter math |
| `project/track.rs` | 97 | `TrackHandle` (engine) + `Track` (project) definitions |
| `project/automation.rs` | 61 | `AutomationLane` + `AutomationPoint` |
| `project/clip.rs` | 76 | `AudioClip` with waveform peaks |
| `project/clip_handle.rs` | 40 | `ClipHandle` (engine-side) |
| `project/marker.rs` | 19 | `Marker` definition |
| `project/pool.rs` | 18 | `AudioPoolEntry` definition |

## Common Patterns to Follow

### Adding a New Feature
1. Check if it modifies clip/track/effect state → must update BOTH models
2. Check if it touches the audio callback → use atomics, thread_local buffers, `try_lock()`
3. Check if it needs serialization → add `Serialize`/`Deserialize` to relevant structs, update `load_project_file`
4. Check if it adds UI layout values → store in `PreferencesState` + `TimelineState`, thread as parameters

### File Sizing Rules
- **Keep files under 250 lines.** If exceeding, split into focused sub-modules.
- **Extract helper functions** when a function exceeds 40 lines.
- **Extract interaction logic** from rendering code.
- UI code should NOT contain DSP math — put that in `dsp/`.

### Adding Effects
1. Add variant to `EffectType` enum in `dsp_effect.rs`
2. Create effect struct implementing `DspEffect` + `Parameterizable`
3. Add factory entry in `effects/mod.rs` `create_effect()`
4. Reuse `dsp/biquad.rs` for filter-based effects

### Timeline Layout Parameters
When adding new timeline rendering or interaction code:
- Accept `header_width: f32` and `track_height: f32` as parameters (do NOT import constants)
- Get them from `app.timeline_state.header_width` / `app.timeline_state.track_height`
- See `timeline/mod.rs::render()` for the canonical pattern of threading these through

## What NOT To Do
- Don't allocate `Vec` in audio callback — use `thread_local!` scratch buffers
- Don't add new `Mutex` in the audio path — use atomics or lock-free structures
- Don't modify the audio engine's `Stream` after initialization
- Don't assume project and engine models are in sync — validate indices before unwrapping
- Don't add dependencies not already in `Cargo.toml` without approval
- Don't use `FileDialog::directory()` — use `initial_directory(path.clone())` (takes `PathBuf`, not `&PathBuf`)
- Don't create compile-time constants for layout values that should be runtime-configurable
- Don't remove the `ui.horizontal()` wrapper in `mixer_panel::render` — mixer channels (master + tracks) must be laid out side-by-side, not stacked vertically

## Key Dependencies
- egui/eframe 0.30 — UI
- cpal 0.15 — Audio I/O
- dasp 0.11 — Audio types
- hound 3.5 — WAV loading
- egui_file_dialog 0.8 — File dialogs
- ron 0.8 — Project serialization
- uuid 1 — Clip/track IDs
- serde 1.0 — Serialization derive macros
- tracing 0.1 — Structured logging
