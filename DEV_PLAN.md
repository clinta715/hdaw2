# HDAW — Development Roadmap

## Vision
A lightweight, performant DAW inspired by classic Cubase (SX 3–6 era), built in Rust with egui. Targeted at composers, sound designers, and musicians who want a fast, no-nonsense DAW with modern CLAP plugin hosting.

---

## Gap Analysis (vs Cubase SX 3–6)

### Parity Features (Already Present)
- Multi-track timeline with audio + MIDI clips
- Basic transport (play/pause/stop/loop)
- Clip drag, trim, snap
- Track mute/solo
- Volume + pan per track
- Automation lanes (volume, pan)
- CLAP plugin hosting (instruments + effects)
- Built-in effects (gain, EQ, delay, reverb, compressor)
- Save/load project via RON
- Undo/redo
- WAV import with sample rate conversion
- MIDI file import
- Piano roll (basic)
- Pool clip management
- Time ruler with markers
- Preferences system (persisted)

### Missing Features

#### Tier 1 (Essential — Phase 1a/b–5)
| Feature | Priority | Phase | Status |
|---------|----------|-------|--------|
| Clip Fades & Crossfades | P0 | 1a | ✅ Done |
| Metronome (click track) | P0 | 1b | ✅ Done |
| Audio Recording (disk-streaming) | P0 | 2 | ✅ Done |
| Audio Mixdown (export to WAV) | P0 | 3 | ✅ Done |
| Audio Routing (Groups + Send FX) | P0 | 5 | ⬜ Not started |
| Tempo Track (multiple tempo/time-sig events) | P0 | 4 | ✅ Done |

#### Tier 2 (Important)
| Feature | Notes |
|---------|-------|
| MIDI recording | Requires duplex audio + recording infrastructure |
| MIDI file export | Requires Tempo Track first for accurate timing |
| Quantization | MIDI + audio clip quantization |
| Time-stretch / pitch-shift | Audio clip varispeed |
| Comping / take lanes | Multi-take recording management |
| Groove quantize / templates | Advanced MIDI quantization |
| Track folders / busses | Hierarchical track organization |
| Marker track (arranger) | Cubase-style arranger markers |
| Nudge / move by grid | Keyboard shortcuts for clip positioning |
| Track presets | Save/load track FX chains |

#### Tier 3 (Nice-to-have)
| Feature | Notes |
|---------|-------|
| Media browser / loop browser | Built-in loop/file browser |
| MIDI editor advanced | CC lanes, drum editor |
| Score editor | Notation view |
| Audio Pitch Correction | Real-time pitch correction |
| Spectrum analyzer / FFT | In-effect editor |
| Sidechain routing | Sidechain input for compressors |
| Video import / scoring | Video playback + timecode |
| VST3 support | Currently CLAP-only, VST3 via clap-vst3 bridge |
| ReWire / ReaMote | Inter-DAW connectivity |
| Export to MP3/FLAC/Ogg | Beyond WAV-only |

---

## Architecture Principles

### Real-Time Audio Thread Rules
- **No heap allocations** in audio callback — use `thread_local!` scratch buffers
- **No locks** — only `try_lock()`. UI uses blocking `lock()`
- **Atomics only** for inter-thread communication
- Thread naming: `hdaw-audio` via `SetThreadDescription`

### Dual-Model Sync
- **Project model** — serializable, UI-facing
- **Engine model** — real-time, atomics
- Every operation must update **both** models

### File Sizing
- Keep files under 350 lines
- Extract helper functions > 40 lines
- Separate interaction from rendering

---

## Implementation Phases

### Phase 1a: Clip Fades & Crossfades ✅
- `AudioClip.fade_in_frames` / `fade_out_frames` (project model)
- `ClipHandle.fade_in_frames` / `fade_out_frames` via `AtomicU64` (engine model)
- `compute_fade_gain()` per-sample gain curve in `process.rs`
- Fade handle drag interaction in timeline
- FadeClip undo command
- Serialization in `sync_engine_to_project()` / `load_project_file()`

### Phase 1b: Metronome
- Sine oscillator metronome click
- `metronome_enabled` atomic on Transport
- BPM atomic on Transport (synced from project)
- Beat-accurate click generation after `master_bus.process()`
- Toolbar toggle button (♩ icon)
- Configurable metronome settings (volume, time-sig accent)

### Phase 2: Audio Recording
- Duplex CPAL stream (input + output)
- Lock-free SPSC ring buffer for input capture
- Disk-streaming WAV writer via `hound::WavWriter` (incremental append)
- Recording state machine (idle → count-in → recording → finishing)
- Clip creation on stop: engine clip with backing buffer + project audio clip
- Arm button per track, global record button
- Optional pre-roll / count-in bars
- Pool integration (recorded clips appear in audio pool)

### Phase 3: Audio Mixdown (Export) ✅
- Offline render that reuses `process_track` with manual position counter (no Transport side-effects)
- `render_export()` in `audio/stream.rs` — processes all tracks for a frame range into interleaved f32
- 32-bit float intermediate → scaled to target bit depth on write via `hound::WavWriter`
- WAV output: 16/24-bit int or 32-bit float, stereo
- Export range: full project (auto-calculated from max clip end, via `project_length_frames()`) or loop region
- File dialog for save path (File → Export Audio...)
- Modal export dialog with bit depth selection, loop-range checkbox, and progress bar with Cancel
- Export lifecycle: `export_requested` → file dialog → settings → `exporting` flag → synchronous render → done message
- Blocks audio thread during export via track lock contention

### Phase 4: Tempo Track
- `TempoEvent` model: `{ tempo: f64, position_frames: u64 }`
- `TimeSigEvent` model: `{ numerator: u8, denominator: u8, position_frames: u64 }`
- `Project.tempo_events: Vec<TempoEvent>`, `Project.time_sig_events: Vec<TimeSigEvent>`
- `tempo_at(position)` / `time_sig_at(position)` methods
- Default single event at frame 0
- Ruler rewrite to iterate tempo spans for tick positions
- Beat-to-frame / frame-to-beat conversion using tempo track
- Tempo track editor (add/edit/delete events in timeline area)
- Transport BPM atomic updated from tempo track at seek/playhead position
- MIDI import tempo map support
- Metrognome uses tempo track for beat timing

### Phase 5: Audio Routing (Groups + Send FX)
- `TrackKind` enum: `Normal | Group(Vec<usize>) | Send(Vec<SendSlot>)`
- `SendSlot`: `{ target_track: usize, level: AtomicU32, pre_fader: bool }`
- Multi-pass `mix_tracks()`: process normal → accumulate group busses → mix sends
- Bus `MasterBus`-like accumulator per group
- Send level/pan per slot
- Routing graph validation (no cycles)
- Group track UI (collapsible children, bus meters)
- Send FX inserts with mix knob
- Serialization of routing topology

---

## Dependency/Phase Ordering

```
Phase 1a (Fades) ──→ done
Phase 1b (Metronome) ──┐
                       ├──→ all independent, can parallelize
Phase 4 (Tempo Track) ─┘
       │
       ▼
Phase 2 (Recording) ──→ needs duplex audio
       │
       ▼
Phase 3 (Export) ──────→ needs nothing else
       │
       ▼
Phase 5 (Routing) ─────→ needs groups concept
```

Phase 1b and Phase 4 were implemented together (Phase 4 provides the tempo model, Phase 1b uses BPM from transport). Both are complete.

---

## Key Design Decisions

1. **Disk-streaming recording from day one** — no RAM-only intermediate step. Uses `hound::WavWriter` for incremental writes, SPSC ring buffer for lock-free capture.
2. **Groups + Send FX in single phase** — they share routing infrastructure.
3. **Tempo Track in Tier 1** — affects ruler, timeline grid, metronome, MIDI timing. Foundational.
4. **WAV-only export initially** — 16/24/32-bit, mono/stereo. Other formats can wrap libav.
5. **Fades as frame counts** — stored in both models, `AtomicU64` for lock-free audio access.
6. **Crossfade by overlap** — sum of fading clips naturally creates crossfades; no special handling needed.
7. **No MIDI recording in Phase 1** — deferred to Tier 2 alongside MIDI file export.

---

## Completed Work (May 2026)

### Phase 1a: Clip Fades & Crossfades ✅
- [x] `AudioClip.fade_in_frames` / `fade_out_frames` with `#[serde(default)]`
- [x] `ClipHandle` atomic fade fields, default to 0
- [x] `compute_fade_gain()` in `audio/process.rs`
- [x] Per-sample fade gain in clip loop
- [x] Fade handle rendering (triangles + fade region overlay)
- [x] Fade drag interaction (hit-test + FadeIn/FadeOut DragMode)
- [x] `update_clip_fade()` command (dual-model sync)
- [x] `FadeClip` undo command
- [x] Fade serialization on save (`sync_engine_to_project`)
- [x] Fade restoration on load (`load_project_file`)

### Phase 1b: Metronome ✅
- [x] `Transport.metronome_enabled: AtomicBool` with toggle method
- [x] BPM + time sig atomics on Transport, synced from project
- [x] Sine-wave metronome click (1kHz, 10ms burst) in `mix_tracks()`
- [x] Metronome toggle button (♩) in toolbar
- [x] BPM/time-sig synced every frame from project → transport

### Phase 4: Tempo Track ✅
- [x] `TempoEvent` / `TimeSigEvent` model structs in `project/tempo_event.rs`
- [x] `tempo_at()` / `time_sig_at()` query functions
- [x] `frames_to_beats()` / `beats_to_frames()` conversion functions
- [x] `Project.tempo_events` / `Project.time_sig_events` with `#[serde(default)]`
- [x] Ruler rewritten for tempo-aware tick positions and labels
- [x] Grid lines rewritten for tempo-aware spacing
- [x] Tempo change indicators (orange markers at event positions)
- [x] Transport BPM updated from tempo track at current position

### Phase 2: Audio Recording ✅
- [x] `TrackHandle.armed: AtomicBool` + `TrackUiState.armed` for UI
- [x] `audio/record.rs` — dedicated recording worker thread via `std::sync::mpsc::sync_channel`
- [x] Recording worker writes 32-bit float WAV incrementally via `hound::WavWriter`
- [x] `build_input_stream()` in `audio/stream.rs` — separate CPAL input stream
- [x] `AudioEngine.recording: Mutex<Option<RecordingSession>>` with start/stop methods
- [x] Record button (●) in toolbar with pulsing red indicator when recording
- [x] Arm button (R) per track header
- [x] `toggle_track_arm()` command
- [x] `start_recording()` / `finish_recording()` lifecycle in `HdawApp`
- [x] Recorded clips created on all armed tracks at recording start position
- [x] Audio pool entry for each recording
- [x] Undo support for RecordAudio (removes clips on undo)
- [x] Recording saved to `{project_dir}/Audio Recordings/` directory
- [x] Stop/pause during recording also stops recording
