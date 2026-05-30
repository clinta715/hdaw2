# HDAW Architecture Guide for AI Agents

## Core Architecture

### Dual-Model Sync (Critical Architectural Debt)
Two parallel data models must be manually kept in sync:
- **Project model** (`Project`, `Track`, `ClipKind::Audio(AudioClip) | ClipKind::Midi(MidiClip)`, `AutomationLane`) — serializable, used for save/load and UI display
- **Engine model** (`TrackHandle`, `ClipHandle.midi_notes: Vec<MidiNote>`, `EffectInstance`) — real-time, uses atomics, used by audio callback

**Every operation that modifies a clip/track/effect must update BOTH models.** The sync is ad-hoc:
- `app/commands.rs` — `update_clip_position`, `update_clip_trim`, `remove_selected_clip`, `add_midi_note`, `remove_midi_note`, `add_midi_clip`, `delete_track` touch both
- `app/project_io.rs` — `sync_engine_to_project` reads engine → project for serialization (includes MIDI note sync); `load_audio_file` resamples to engine sample rate
- `ui/timeline/auto_interaction.rs` — `sync_automation_to_project` copies engine auto points → project model (uses dirty flag)
- `ui/effect_editor/` — all FX operations (`add_builtin_effect`, `add_clap_effect`, `remove_effect`, `write_bypass`, `write_param`) sync to project model via `engine_fx_to_serialized()` helper
- MIDI clip editing updates both project `MidiClip.notes` and engine `ClipHandle.midi_notes` atomically for real-time playback

### Real-Time Audio Thread Rules
- **NO heap allocations in audio callback.** Use `thread_local!` scratch buffers (`stream.rs`, `process.rs`, `clap_effect.rs`) that `resize()` instead of allocating new `Vec`.
- **NO locks in audio callback.** Use `try_lock()` only. The UI uses blocking `lock()`.
- Thread naming: `hdaw-audio` on Windows via `SetThreadDescription` (one-time in callback via `thread_local! Cell<bool>`).
- ~19 `lock()` calls from UI code into the engine `Arc<Mutex<Vec<TrackHandle>>>` — lock contention risk.

### Audio Pipeline
`audio_callback()` → reads/clears `seek_occurred` → `stream::name_audio_thread()` → `stream::mix_tracks(seek_occurred)` (per-track: automation → clips → MIDI dispatch → FX chain → mix sum) → `master_bus.process()` → interleave to output

### Atomics for Real-Time Safety
| Type | Use | Access |
|------|-----|--------|
| `AtomicU32` | f32 params (volume, pan, gain) | `f32::to_bits`/`from_bits` |
| `AtomicBool` | mute, solo, bypass, `seek_occurred` | `Ordering::Acquire`/`Release` |
| `AtomicU64` | position frames, packed loop region | audio reads, UI writes |

### CLAP Plugin Hosting Architecture
- **Scanner** (`audio/clap_scanner.rs`): Discovers `.clap` plugin files in OS-standard directories, loads entry points via `clack_host::PluginEntry::load()`, extracts `PluginDescriptor` metadata (name, id, features, is_instrument)
- **Host** (`audio/clap_host.rs`): Implements `HostHandlers` for `clack_host`, provides `HdawClapHost` with logging via `tracing`
- **Plugin State** (`audio/clap_instance.rs`): `ClapPluginState` holds plugin metadata, parameter info/values, bypass state. Parameters bridged to HDAW's `AtomicU32` pattern for lock-free audio thread access
- **Effect Adapter** (`audio/clap_effect.rs`): `ClapEffectAdapter` wraps a `ClapPluginState` behind `Mutex` for thread safety. Implements `Drop` (calls `deactivate()`). Uses `try_lock()` on `pending_params` in `process_inner()`. `process()` calls `process_inner()` with `InputEvents::empty()`, `process_with_events()` passes caller-supplied events. Detects note-input capability via `PluginNotePorts` extension query at load time. Thread-local `COMBINED_EVENTS` buffer for merging pending + input events.
- **EffectKind enum** in `dsp_effect.rs`: `EffectInstance.kind` is either `BuiltIn(Box<dyn DspEffect>)` or `Clap(Mutex<ClapEffectAdapter>)`. All `EffectInstance` methods dispatch based on variant. CLAP variant uses poison-safe `lock_clap()`/`lock_clap_mut()` helpers.
- **Transport**: Play/Pause/Stop — `pause()` preserves position, `stop()` resets to zero. `Space` = play/pause toggle
- **EffectType** has a `Clap { plugin_id, path }` variant for serialized CLAP plugin references

### Transport Architecture
- `Transport.playing: AtomicBool` — play/pause/stop via `play()`, `pause()`, `stop()`
- **Play** sets `playing=true`
- **Pause** sets `playing=false` (preserves position)
- **Stop** sets `playing=false` AND resets position to 0
- `loop_region: AtomicU64` — packed as (loop_out << 32) | loop_in to avoid torn reads
- `seek_occurred: AtomicBool` — set by `seek_to_frame()`, cleared once per audio callback via `swap(false, Acquire)`. Triggers NoteOff for all active notes to prevent stuck notes on seek.
- UI triggers via `play_requested`, `pause_requested`, `stop_requested` flags

### Automation Architecture
- `AutomationLane` in both project and engine models
- Volume + Pan lanes auto-created per track (sentinel IDs `PARAM_VOLUME`, `PARAM_PAN`)
- `process_track()` evaluates automation per buffer using `get_value_at(pos)`
- f32::NAN from empty lane = use manual atomic value
- Local-override: automation curves don't overwrite atomics; result used locally per buffer
- **Dirty flag pattern**: `AutomationLane.dirty: bool` (`#[serde(skip)]`) — set on mutation (`add_point`, `remove_point`, drag), checked by `sync_automation_to_project` to skip redundant deep-clones

### MIDI Architecture (v0.3.0)
- **Data model**: `MidiNote` (pitch, velocity, start_frame, duration) + `MidiClip` (id, name, position/length, notes, color)
- **ClipKind enum**: `ClipKind::Audio(AudioClip) | ClipKind::Midi(MidiClip)` unifies both clip types on the project model
- **Engine model**: `ClipHandle.midi_notes: Vec<MidiNote>` — separate from audio data, empty for audio clips
- **Playback**: `process_track()` scans `ClipHandle.midi_notes` in each clip, builds sorted `EventBuffer` of NoteOn/NoteOff, sends to the first note-capable CLAP effect in the FX chain via `process_with_events()`
- **Seek handling**: On `seek_occurred`, sends NoteOff at offset 0 for all pre-existing notes; sends NoteOn for any note active in the buffer even if it started before the buffer
- **Instrument detection**: `has_note_input` flag on `EffectInstance`; `ClapEffectAdapter` queries `PluginNotePorts` extension at load time
- **Instrument slot**: first effect in FX chain with `has_note_input == true`; skipped in standard FX loop
- **Piano roll**: `ui/piano_roll.rs` — grid editor with note add (left-click), delete (right-click), playhead indicator
- **Sync**: `add_midi_note` / `remove_midi_note` / `add_midi_clip` in `commands.rs` update both project and engine models, push undo commands
- **Event sorting**: MIDI events sorted by sample offset using `Vec::sort()` on the EventBuffer before dispatch
- **Default note duration**: 1 beat (computed from BPM * sample rate)
- **Sample rate aware**: `ClipHandle::new_midi()` takes `sample_rate` param; `draw_midi` takes `sample_rate` for correct beat-to-frame conversion
- **No MIDI recording** or file import/export in Phase 1

### Undo Architecture (v0.3.0)
- **UndoCommand enum** in `undo/mod.rs`: `MoveClip`, `TrimClip`, `DeleteClip`, `AddMidiNote`, `RemoveMidiNote`, `AddMidiClip`, `AddTrack`, `DeleteTrack`
- `apply_undo`/`apply_redo` take `&mut [TrackHandle]` (slice) — cannot add/remove elements
- **Track undo/redo** handled at `HdawApp` level in `undo()`/`redo()` with full `Vec<TrackHandle>` access. `AddTrack`/`DeleteTrack` are no-op in `apply_undo`/`apply_redo`.
- `DeleteTrack` undo restores track + track UI state + returns clips from pool
- `AddTrack` undo removes the track from both models
- `DeleteClip` undo creates `ClipHandle::new_midi()` for MIDI clips
- `PoolClip::from_clip()` preserves original clip UUID for undo consistency

### Effect Parameter Pattern
- `DspEffect` trait: `process(&mut self)` (mutable DSP) + `Parameterizable` (immutable reads via `ParameterValue`)
- `ParameterValue` wraps `AtomicU32` for lock-free audio thread reads
- Parameter changes: UI calls `effect.set_parameter()` → atomic store → effect reads on next `process()` call
- Dirty flag pattern on EQ: marks coeffs need rebuild, rebuilds on next `process()` call

### Thread-Local Scratch Buffers
```
stream.rs:    SCRATCH_L, SCRATCH_R    — output accumulation
process.rs:   MIX_L, MIX_R            — per-track clip mixing
clap_effect.rs: COMBINED_EVENTS       — merged pending + input CLAP events
```
These are `thread_local! RefCell<Vec<f32>>` (or `EventBuffer`) that grow on first use but stabilize capacity after a few callbacks.

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

### Sample Rate Conversion
- `resample()` in `project_io.rs` — linear interpolation resampler
- `load_audio_file()` detects WAV file rate mismatch with engine rate and resamples automatically
- Keeps all clips at the engine's native sample rate for consistent playback

## File Map (Current)

| File | Lines | Purpose |
|------|-------|---------|
| `app/mod.rs` | 334 | HdawApp struct, constructor, accessors, `eframe::App::update`, undo/redo with track handling |
| `app/commands.rs` | 399 | Clip/track manipulation ops, pool clip restore, MIDI note/clip add/remove, track delete with undo |
| `app/project_io.rs` | 308 | Save/load/new/import, resample, sync_engine_to_project (incl. MIDI note sync) |
| `app/input.rs` | 158 | Keyboard handling, pending requests, file dialogs |
| `app/prefs_io.rs` | 34 | Preferences load/save via RON |
| `app/undo/mod.rs` | 135 | UndoCommand enum (incl. AddTrack/DeleteTrack/AddMidiNote/RemoveMidiNote/AddMidiClip), UndoStack |
| `app/undo/commands.rs` | 343 | apply_undo/apply_redo implementations for all clip/MIDI variants |
| `ui/timeline/mod.rs` | 305 | Timeline render, zoom/scroll, grid, track layout, context menu |
| `ui/timeline/clips.rs` | 282 | Clip waveform/MIDI drawing + drag/trim/double-click interaction |
| `ui/timeline/automation.rs` | 173 | Automation curve drawing + point editing helpers |
| `ui/timeline/interaction.rs` | 206 | Seek, loop, clip, track header interaction handlers |
| `ui/timeline/auto_interaction.rs` | 146 | Automation point interaction + dirty-flag sync to project |
| `ui/timeline/ruler.rs` | 121 | Time ruler ticks, labels, loop markers |
| `ui/timeline/track_headers.rs` | 139 | Track header drawing, mute/solo buttons, hit testing |
| `ui/timeline/playhead.rs` | 14 | Playhead line drawing |
| `ui/piano_roll.rs` | 220 | Piano roll grid editor with MIDI note add/delete/playhead |
| `ui/effect_editor/mod.rs` | 337 | FX chain panel + effect parameter UI + engine_fx_to_serialized sync |
| `ui/effect_editor/eq_graph.rs` | 82 | EQ frequency response graph |
| `ui/preferences.rs` | 206 | Preferences dialog (3 sections: Audio, Project, Timeline/UI) |
| `ui/toolbar.rs` | 169 | Top toolbar with transport controls, menus, "+" dropdown |
| `ui/app_ui.rs` | 175 | Main layout: toolbar, panels, timeline, status bar, instrument dialog |
| `ui/mixer_panel.rs` | 94 | Mixer strip panel (VU meter + slider side-by-side) |
| `ui/audio_pool.rs` | 83 | Audio pool panel for imported files |
| `audio/engine.rs` | 134 | AudioEngine struct, init, play/pause/stop, rebuild, seek_occurred pass-through |
| `audio/stream.rs` | 150 | build_stream, mix_tracks with seek_occurred, name_audio_thread, scratch buffers |
| `audio/process.rs` | 119 | Per-track audio processing with seek-aware MIDI dispatch |
| `audio/transport.rs` | 80 | Transport: play/pause/stop, packed loop region, position, seek_occurred |
| `audio/clap_scanner.rs` | 100 | CLAP plugin discovery in OS-standard directories |
| `audio/clap_host.rs` | 47 | HDAW CLAP host handlers (logging, lifecycle) |
| `audio/clap_instance.rs` | 75 | ClapPluginState: plugin metadata, parameter bridge |
| `audio/clap_effect.rs` | 180 | ClapEffectAdapter: Drop impl, try_lock, COMBINED_EVENTS, process_inner, note-port detection |
| `audio/mixer.rs` | 43 | Master bus volume + peak metering |
| `audio/effects/` | ~540 | 5 effects (Gain, EQ, Delay, Reverb, Compressor) + traits + factory |
| `dsp/biquad.rs` | 89 | Shared biquad filter math |
| `project/track.rs` | 97 | `TrackHandle` (engine) + `Track` (project) with `clips: Vec<ClipKind>` |
| `project/automation.rs` | 64 | `AutomationLane` (with dirty flag) + `AutomationPoint` |
| `project/clip.rs` | 88 | `ClipKind` enum + `AudioClip` with waveform peaks |
| `project/clip_handle.rs` | 69 | `ClipHandle` (engine-side) with `midi_notes`, sample-rate-aware `new_midi()` |
| `project/midi_clip.rs` | 13 | `MidiClip` struct (position, length, notes, color) |
| `project/midi_note.rs` | 9 | `MidiNote` struct (pitch, velocity, start_frame, duration) |
| `project/marker.rs` | 19 | `Marker` definition |
| `project/pool.rs` | 28 | `AudioPoolEntry` definition (supports ClipKind, preserves UUID) |

## Common Patterns to Follow

### Adding a New Feature
1. Check if it modifies clip/track/effect state → must update BOTH models
2. Check if it touches the audio callback → use atomics, thread_local buffers, `try_lock()`
3. Check if it needs serialization → add `Serialize`/`Deserialize` to relevant structs, update `load_project_file`
4. Check if it adds UI layout values → store in `PreferencesState` + `TimelineState`, thread as parameters
5. Check if it needs undo → add variant to `UndoCommand`, implement in `apply_undo`/`apply_redo`; for track-level ops handle at `HdawApp` level

### Adding Undo for a New Operation
1. Add variant to `UndoCommand` enum in `undo/mod.rs`
2. Implement restore logic in `apply_undo()` and `apply_redo()` in `undo/commands.rs`
3. If the operation affects track count, handle at `HdawApp::undo()`/`redo()` level instead (slice-based `apply_undo` can't add/remove elements)
4. Push the command via `self.undo_state.push(UndoCommand::...)` at the call site
5. Preserve UUIDs when creating pool clips or restoring from undo (use `PoolClip::from_clip` which preserves original IDs)

### File Sizing Rules
- **Keep files under 350 lines.** If exceeding, split into focused sub-modules.
- **Extract helper functions** when a function exceeds 40 lines.
- **Extract interaction logic** from rendering code.
- UI code should NOT contain DSP math — put that in `dsp/`.

### Adding Effects
1. Add variant to `EffectType` enum in `dsp_effect.rs`
2. Create effect struct implementing `DspEffect` + `Parameterizable`
3. Add factory entry in `effects/mod.rs` `create_effect()`
4. Reuse `dsp/biquad.rs` for filter-based effects
5. If adding CLAP effect via UI, sync to project model using `engine_fx_to_serialized()` pattern

### Timeline Layout Parameters
When adding new timeline rendering or interaction code:
- Accept `header_width: f32` and `track_height: f32` as parameters (do NOT import constants)
- Get them from `app.timeline_state.header_width` / `app.timeline_state.track_height`
- See `timeline/mod.rs::render()` for the canonical pattern of threading these through

### Mutex Poison Recovery
- Use `lock_clap()`/`lock_clap_mut()` helpers in `dsp_effect.rs` for CLAP effect access
- These use `.lock().unwrap_or_else(|e| e.into_inner())` to recover from poisoned mutexes
- Never `unwrap()` a `Mutex::lock()` result in production code

## What NOT To Do
- Don't allocate `Vec` in audio callback — use `thread_local!` scratch buffers
- Don't add new `Mutex` in the audio path — use atomics or lock-free structures
- Don't modify the audio engine's `Stream` after initialization
- Don't assume project and engine models are in sync — validate indices before unwrapping
- Don't add dependencies not already in `Cargo.toml` without approval
- Don't use `FileDialog::directory()` — use `initial_directory(path.clone())` (takes `PathBuf`, not `&PathBuf`)
- Don't create compile-time constants for layout values that should be runtime-configurable
- Don't remove the `ui.horizontal()` wrapper in `mixer_panel::render` — mixer channels (master + tracks) must be laid out side-by-side, not stacked vertically
- Don't use `drop(())` for unused values — use `let _ = ()` instead
- Don't generate new UUIDs in `PoolClip::from_clip()` — preserve the original clip's UUID for undo consistency
- Don't deep-compare automation points every frame — use the dirty flag pattern
- Don't use `.lock().unwrap()` on CLAP effect mutexes — use poison-safe `lock_clap()`/`lock_clap_mut()` helpers

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
- clack_host 0.1 — CLAP plugin hosting
