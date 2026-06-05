# HDAW Architecture Guide for AI Agents

## v0.9.0 Changes (Tiled Layout + Loop/Panel Persistence)

### Tiled Layout (Ableton Live-style)
- **Main view dispatch** (`app/mod.rs`, `app_ui.rs`): `MainView::Arrange | PianoRoll` enum replaces `show_piano_roll: bool`. `CentralPanel` dispatches based on `app.main_view`. Piano roll renders inline via `render_panel(ui, app)` instead of in a floating `Window`.
- **Right panel** (`right_panel.rs`): New `SidePanel::right` with tabbed modes — Browser (audio pool), Clip Info, Effect Detail. Width saved to `preferences.right_panel_width` via `PanelResponse.response.rect.width()`.
- **Bottom panel** (`bottom_panel.rs`): New `TopBottomPanel::bottom` replacing `mixer_panel.rs`. Tabs for Mixer, Sends, FX Chain. Mixer strips migrated from deleted `mixer_panel.rs`. Height saved to `preferences.mixer_panel_height`.
- **`Panels` enum pruned** (`panels.rs`): `PianoRoll` and `Mixer` variants removed. Only floating panels remain (AudioPool, EffectEditor, Preferences).
- **`mixer_panel.rs` deleted**: All mixer strip/draw_master/draw_channel/VU meter logic moved to `bottom_panel.rs`.

### Loop Region Persistence
- **Project model** (`project/mod.rs`): Added `loop_in_frames: u64`, `loop_out_frames: u64`, `loop_enabled: bool` — all `#[serde(default)]` for backward compat.
- **Sync on save** (`project_io.rs`): `sync_engine_to_project()` copies loop region + enabled from engine transport to project.
- **Restore on load** (`project_io.rs`): After loading project, restores `set_loop_region()` + `loop_enabled.store()` onto transport.

### Panel Size Persistence
- **Right panel width**: New `right_panel_width: f32` in `PreferencesState` (default 220.0). Read on render, saved every frame from `PanelResponse`.
- **Bottom panel height**: Already stored in `mixer_panel_height` — no change needed.
- **Auto-save on close**: `app_ui.rs` triggers `save_preferences()` on `ctx.input(|i| i.viewport().close_requested())` before any unsaved-changes dialog.

### Bug Fixes
- **Empty project save prompt**: Added `mark_saved()` after `add_blank_track()` in `HdawApp::new()`.
- **Piano roll window growth**: Cached `initial_window_size` via `get_or_insert_with()`, added `.min_size(400, 300)`.

## v0.8.0 Changes (Expanded Track View + Audio Engine Performance)

### Expanded Track View
- **Track expand button** (`track_headers.rs`): ⇕ button on track header toggles `app.expanded_track: Option<usize>`.
- **Stacked automation lanes** (`mod.rs`): 60px rows for Volume, Pan, effect-param lanes.
- **Velocity lane** (`mod.rs`): 60px MIDI note velocity bars along timeline. Click-drag to edit with undo via `UpdateMidiNote`.
- **Variable-height track Y positions**: `compute_track_y_positions()`/`track_idx_from_y()` helpers replace fixed `idx * track_height`.

### Audio Engine Performance
- **SPSC lock-free parameter pipeline** (`param_ring.rs`, `clap_effect.rs`): `ParamRingBuffer` with `UnsafeCell<Vec<ParamChange>>` and `AtomicU64` indices. Eliminates lock contention on `pending_params`.
- **Kahn VecDeque + binary search elimination** (`stream.rs`): `pop_front()` O(1) vs `remove(0)` O(n). 5 `binary_search()` → `HashMap` lookups.
- **Clippy cleanup**: 97 warnings + 1 error → 0.

## Core Architecture

### Dual-Model Sync (Critical Architectural Debt)
Two parallel data models must be manually kept in sync:
- **Project model** (`Project`, `Track`, `ClipKind`, `AutomationLane`) — serializable, save/load
- **Engine model** (`TrackHandle`, `ClipHandle.midi_notes`, `EffectInstance`) — real-time, atomics

**Every operation that modifies a clip/track/effect must update BOTH models.**

### Real-Time Audio Thread Rules
- **NO heap allocations in audio callback.** Use `thread_local!` scratch buffers that `resize()` instead of allocating.
- **NO locks in audio callback.** Use `try_lock()` only. UI uses blocking `lock()`.
- Thread naming: `hdaw-audio` on Windows via `SetThreadDescription`.

### Audio Pipeline
`audio_callback()` → reads/clears `seek_occurred` → `stream::mix_tracks(seek_occurred)` (per-track: automation → clips → MIDI dispatch → FX chain → mix sum) → `master_bus.process()` → interleave to output

### Atomics for Real-Time Safety
| Type | Use | Access |
|------|-----|--------|
| `AtomicU32` | f32 params (volume, pan, gain) | `f32::to_bits`/`from_bits` |
| `AtomicBool` | mute, solo, bypass, `seek_occurred` | `Ordering::Acquire`/`Release` |
| `AtomicU64` | position frames, packed loop region | audio reads, UI writes |

### Transport Architecture
- `Transport.playing: AtomicBool` — play/pause/stop
- `loop_region: AtomicU64` — packed as (loop_out << 32) | loop_in
- `seek_occurred: AtomicBool` — set by `seek_to_frame()` and `stop()`, cleared once per audio callback. Triggers NoteOff for all active notes + CLAP reset.

### Preferences System
- `PreferencesState` in `ui/preferences.rs` — audio config, project defaults, UI layout values
- Persisted via RON to `%APPDATA%/hdaw/preferences.ron`
- Saved on: Apply in dialog, file dialog directory changes, and `close_requested()`
- Panel sizes (`right_panel_width`, `mixer_panel_height`) stored in prefs

### Loop Region Persistence
- Loop in/out frames + enabled flag stored on `Project` struct with `#[serde(default)]`
- `sync_engine_to_project()` copies from `Transport.load_loop_region()` + `loop_enabled` to project model
- `load_project_file()` calls `Transport.set_loop_region()` + `loop_enabled.store()` after project load
- **Always use `#[serde(default)]`** when adding new fields to `Project` — old .ron files won't have them

### Panel Layout (Tiled)
- `SidePanel::right` → Right panel (Browser/Clip Info/FX Detail)
- `CentralPanel` → Main tile, dispatches on `MainView::Arrange | PianoRoll`
- `TopBottomPanel::bottom` → Bottom panel (Mixer/Sends/FX Chain)
- `TopBottomPanel::bottom` → Status bar (absolute bottom)
- Panel sizes restored from `PreferencesState` via `default_width()` / `default_height()`
- Sizes saved every frame from `PanelResponse.response.rect.{width,height}()`

## File Map (Current)

| File | Lines | Purpose |
|------|-------|---------|
| `app/mod.rs` | ~334 | HdawApp struct, MainView/RPanelMode/BPanelMode enums, undo/redo |
| `app/commands.rs` | ~399 | Clip/track ops, MIDI note/CC ops, pool restore |
| `app/project_io.rs` | ~167 | Save/load/new/import, sync_engine_to_project, loop sync |
| `app/input.rs` | ~158 | Keyboard handling, pending requests, file dialogs |
| `app/prefs_io.rs` | ~34 | Preferences load/save via RON |
| `app/undo/mod.rs` | ~135 | UndoCommand enum, UndoStack |
| `app/undo/commands.rs` | ~343 | apply_undo/apply_redo implementations |
| `ui/timeline/mod.rs` | ~305 | Timeline render, zoom/scroll, grid, track layout |
| `ui/timeline/clips.rs` | ~599 | Clip drawing + drag/trim/double-click |
| `ui/timeline/automation.rs` | ~173 | Automation curve drawing + editing |
| `ui/timeline/interaction.rs` | ~206 | Seek, loop, clip, header interaction |
| `ui/timeline/auto_interaction.rs` | ~146 | Automation point interaction + dirty-flag sync |
| `ui/timeline/ruler.rs` | ~121 | Time ruler ticks, labels, loop markers |
| `ui/timeline/track_headers.rs` | ~139 | Track header, mute/solo, expand button |
| `ui/timeline/playhead.rs` | ~14 | Playhead line drawing |
| `ui/piano_roll.rs` | ~816 | Piano roll grid editor, controller lanes, render_panel() |
| `ui/piano_roll_state.rs` | ~34 | Controller lane config, drag state |
| `ui/right_panel.rs` | ~118 | Tabbed side panel (Browser/Clip Info/FX) |
| `ui/bottom_panel.rs` | ~343 | Tabbed bottom panel (Mixer/Sends/FX Chain), VU meter |
| `ui/app_ui.rs` | ~391 | Main layout: toolbar → right panel → central → bottom → status bar |
| `ui/toolbar.rs` | ~319 | Transport controls, menus, Arrange toggle, panel toggles |
| `ui/preferences.rs` | ~424 | Preferences dialog + PreferencesState |
| `ui/panels.rs` | ~67 | Remaining floating panels (AudioPool, EffectEditor, Preferences) |
| `ui/audio_pool.rs` | ~83 | Audio pool panel |
| `ui/effect_editor/mod.rs` | ~337 | FX chain panel + parameter UI + engine_fx_to_serialized |
| `ui/effect_editor/eq_graph.rs` | ~82 | EQ frequency response graph |
| `audio/engine.rs` | ~134 | AudioEngine, play/pause/stop, rebuild, loop wrap |
| `audio/stream.rs` | ~150 | build_stream, mix_tracks, scratch buffers |
| `audio/process.rs` | ~119 | Per-track processing with MIDI dispatch |
| `audio/midi_dispatch.rs` | ~155 | MIDI note + CC to CLAP instrument |
| `audio/transport.rs` | ~80 | Transport, loop_region packing, seek_occurred |
| `audio/clap_scanner.rs` | ~100 | CLAP plugin discovery |
| `audio/clap_host.rs` | ~47 | CLAP host handlers |
| `audio/clap_instance.rs` | ~75 | Plugin state, parameter bridge |
| `audio/clap_effect.rs` | ~180 | ClapEffectAdapter, param ring, note-port detection |
| `audio/mixer.rs` | ~43 | Master bus volume + peak metering |
| `audio/effects/` | ~540 | Gain, EQ, Delay, Reverb, Compressor, Chorus, Flanger, Phaser, Distortion |
| `dsp/biquad.rs` | ~89 | Shared biquad filter math |
| `project/` | ~300 | Project/Track/Clip/ClipHandle/MidiNote/MidiClip/CCEvent/Automation/Marker/Pool |
| `tests/midi_pipeline_test.rs` | ~202 | MIDI→CLAP→audio integration tests |

## Common Patterns to Follow

### Adding a New Feature
1. Check if it modifies clip/track/effect state → must update BOTH models
2. Check if it touches the audio callback → use atomics, thread_local buffers, `try_lock()`
3. Check if it needs serialization → add `Serialize`/`Deserialize` to `Project` or `PreferencesState`, use `#[serde(default)]` for backward compat
4. Check if it adds UI layout values → store in `PreferencesState`, thread as parameters
5. Check if it needs undo → add variant to `UndoCommand`, implement `apply_undo`/`apply_redo`

### Adding a New Tiled Panel
1. Choose the right egui panel type: `SidePanel::right`, `TopBottomPanel::bottom`, or `CentralPanel` dispatch
2. Add a mode enum (e.g. `RightPanelMode`) if it has tabs
3. Register the panel's render call in `app_ui.rs` in the correct order
4. Store its default size in `PreferencesState` with a field name
5. Save actual size from `PanelResponse.response.rect.{width,height}()` every frame
6. If replacing a floating Window, provide both `render()` (Window-wrapped) and `render_panel()` (inline) signatures for backward compat

### Preserving State Between Sessions
- **Panel sizes**: Store in `PreferencesState`, read via `default_width()`/`default_height()`, save every frame from `PanelResponse`, persist on `close_requested()`
- **Loop regions**: Store on `Project` with `#[serde(default)]`, sync in `sync_engine_to_project()` and `load_project_file()`
- **File dialog dirs**: `last_import_dir`, `last_open_dir`, `last_save_dir` on `PreferencesState` (Option<PathBuf>)
- **Recent files**: `recent_files: Vec<PathBuf>` on `PreferencesState`, dedup'd, capped at 10

### Floating Panel → Tiled Panel Migration
```rust
// Old: floating window
pub fn render(ctx: &Context, app: &mut HdawApp) {
    if !app.some_flag { return; }
    egui::Window::new("Title").show(ctx, |ui| {
        content(ui, app);
    });
}

// New: both signatures — Window wrapper for backward compat + panel for tiled layout
pub fn render(ctx: &Context, app: &mut HdawApp) {
    if app.some_flag {
        egui::Window::new("Title").show(ctx, |ui| {
            render_panel(ui, app);
        });
    }
}
pub fn render_panel(ui: &mut egui::Ui, app: &mut HdawApp) {
    content(ui, app);
}
```

## What NOT To Do
- Don't allocate `Vec` in audio callback — use `thread_local!` scratch buffers
- Don't add new `Mutex` in the audio path — use atomics or lock-free structures
- Don't modify the audio engine's `Stream` after initialization
- Don't assume project and engine models are in sync — validate indices before unwrapping
- Don't add dependencies not already in `Cargo.toml` without approval
- Don't use `FileDialog::directory()` — use `initial_directory(path.clone())` (takes `PathBuf`, not `&PathBuf`)
- Don't create compile-time constants for layout values that should be runtime-configurable
- Don't use `drop(())` for unused values — use `let _ = ()` instead
- Don't generate new UUIDs in `PoolClip::from_clip()` — preserve original UUID for undo consistency
- Don't deep-compare automation points every frame — use the dirty flag pattern
- Don't use `.lock().unwrap()` on CLAP effect mutexes — use poison-safe `lock_clap()`/`lock_clap_mut()` helpers
- Don't add `#[serde(default)]` fields without testing backward compat — load an old .ron file and verify fields are `Default::default()`
- Don't create new floating window editors — prefer `render_panel(ui, app)` for inline tiling and keep `render(ctx, app)` as a thin `Window` shim

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
- raw-window-handle 0.6 — Native window handle access
