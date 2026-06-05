# HDAW - Project Specification

## 1. Project Overview

**HDAW** (Holofonic Digital Audio Workstation) - A traditional linear DAW similar to Pro Tools/Cubase, built in Rust with egui for the UI.

### Core Philosophy
- Single-window application with persistent panels (not tabbed)
- Arrangement-first workflow (no session view/clips)
- Full feature set from the beginning - no MVP approach
- Audio tracks with waveform display, insert effect chains, per-parameter automation

### Target Platform
- Windows primary (WASAPI via cpal)
- Cross-platform capable (macOS CoreAudio, Linux ALSA/JACK)

---

## 2. Technology Stack

### Audio Engine

| Component | Library | Purpose |
|-----------|---------|---------|
| Audio I/O | `cpal` | Cross-platform audio input/output (WASAPI, CoreAudio, ALSA) |
| DSP Foundation | `dasp` | Low-level signal processing primitives |
| Audio Loading | `audio-file` (neodsp) | Read/write 16+ audio formats (WAV, MP3, FLAC, OGG, AAC) |
| Time-Stretch | `timestretch` | WSOLA + phase vocoder hybrid algorithm |
| FFT | `rustfft` + `spectrum-analyzer` | Frequency analysis for visualization |

### GUI Framework

| Component | Library | Purpose |
|-----------|---------|---------|
| UI Framework | `slint` | Declarative UI, native rendering, cross-platform |
| Styling | Built-in styles (Material/Cupertino/Fluent) | Or custom dark theme |

### Application Logic

| Component | Library | Purpose |
|-----------|---------|---------|
| Serialization | `serde` + `ron` | Project file save/load |
| Async | `tokio` | Background tasks (file loading, processing) |
| Logging | `tracing` | Debugging and diagnostics |

---

## 3. UI Layout

### Single Window Layout

```
+-----------------------------------------------------------------+
|  Menu Bar  (File | Edit | View | Track | Transport | Help)     |
+-----------------------------------------------------------------+
|  Transport Bar                                                  |
|  [<<][>>][Play][Stop][Loop] | 00:00:00.000 | 120.0 BPM | 4/4   |
+-----------------------------------------------------------------+-----------------------------+
|                                                                 |                             |
|                     Timeline / Arrangement                      |       Mixer Panel           |
|  +----------------------------------------------------------+  |  +----+---+---+---+---+----+|
|  | Track 1 ████████████████████████████████████████████████ |  ||Ch1 |Ch2|Ch3|Ch4|Ch5 |Ch6||
|  | Track 2 ███████████████████░░░░░░░█████████████████████ |  || ██ | ██ | ██ | ██ | ██ | ██||
|  | Track 3 ░░░░░░████████████░░░░░░░░░░░░███████████████   |  || ██ | ██ | ██ | ██ | ██ | ██||
|  +----------------------------------------------------------+  |  || ●  | ○  | ●  | ○  | ●  | ○ ||
|                                                                 |  || ▓▓ | ▓▓ | ▓▓ | ▓▓ | ▓▓ | ▓▓||
|  Automation Lanes (collapsible per track)                      |  || ▼  | ▼  | ▼  | ▼  | ▼  | ▼ ||
|  [Volume: =====================================================]  |  ||[=] |[=] |[=] |[=] |[=] |[=] ||
|  [Pan: ==============================================*==========]  |  +----+---+---+---+---+----+|
|                                                                 |         Master             |
+-----------------------------------------------------------------+  +--------+                 |
|                     Mixer Panel                                 |  |  ██   |                 |
|  +------+------+------+------+------+------+    +--------+    |  |  ██   |                 |
|  | Ch1  | Ch2  | Ch3  | Ch4  | Ch5  | Ch6  |    | Master |    |  |  ▓▓   |                 |
|  |  ██  |  ██  |  ██  |  ██  |  ██  |  ██  |    |   ██   |    |  |  ▼    |                 |
|  |  ██  |  ██  |  ██  |  ██  |  ██  |  ██  |    |   ██   |    |  |  [=]  |                 |
|  |  ●   |  ○   |  ●   |  ○   |  ●   |  ○   |    |        |    |  +--------+                 |
|  |  ▓▓  |  ▓▓  |  ▓▓  |  ▓▓  |  ▓▓  |  ▓▓  |    |   ▓▓   |    |                             |
|  |  ▼   |  ▼   |  ▼   |  ▼   |  ▼   |  ▼   |    |   ▼    |    |  Inserts: [EQ][Compressor] |
|  | [=]  | [=]  | [=]  | [=]  | [=]  | [=]  |    |  [=]   |    |  Sends: [A] [B]            |
|  +------+------+------+------+------+------+    +--------+    |                             |
|   Inserts: [EQ][Compressor]  Sends: [A] [B]                  |                             |
+-----------------------------------------------------------------+
|                 Effect Editor / Properties Panel                |
|  [EQ] [Compressor] [Reverb] [Delay]                            |
|  +----------------------------------------------------------+  |
|  | Frequency | Gain | Q                                      |  |
|  | ========================================================== |  |
|  +----------------------------------------------------------+  |
+-----------------------------------------------------------------+
```

### Layout Implementation with egui

```rust
// CentralPanel dispatches main view (Arrange/PianoRoll)
egui::CentralPanel::default().show(ctx, |ui| {
    match app.main_view {
        MainView::Arrange => crate::ui::timeline::render(ui, app),
        MainView::PianoRoll => crate::ui::piano_roll::render_panel(ui, app),
    }
});

// Right panel and bottom panel are tiled via SidePanel/TopBottomPanel
crate::ui::right_panel::render(ctx, app);
crate::ui::bottom_panel::render(ctx, app);
```

---

## 4. Core Architecture

### Audio Engine

```
+---------------------------------------------------------------+
|                      Application Core                        |
+---------------------------------------------------------------+
|  +-------------+  +-------------+  +-------------+            |
|  |  Transport  |  |    Track    |  |   Project   |            |
|  |  Controller |  |   Manager   |  |    State    |            |
|  +------+------+  +------+------+  +------+------+            |
|         |                |                |                  |
|  +------+----------------+----------------+------+            |
|  |              Audio Processing Graph            |            |
|  |                                               |            |
|  |   Track 1 ---+--- [FX Chain] ---+--- Mixer ---+- Master   |
|  |   Track 2 ---|                  |               |            |
|  |   Track 3 ---+                  |               |            |
|  |   ...                          |               |            |
|  +-----------------------+--------+               |            |
|                        |                         |            |
|                   +----+----+                    |            |
|                   |  cpal   |  (Audio Thread)    |            |
|                   |  Stream |                    |            |
|                   +---------+                    |            |
+---------------------------------------------------------------+
```

### Thread Model

| Thread | Responsibility |
|--------|---------------|
| Main Thread | UI rendering, user input, state management |
| Audio Thread | Real-time processing via cpal callback (high priority) |
| Worker Thread | File I/O, time-stretch processing, waveform calculation |

### Real-Time Safety Requirements

- **No heap allocations** in audio callback
- Pre-allocated buffers for all audio processing
- Lock-free communication between threads
- Use `crossbeam` channels with careful design, or atomics-only for audio parameters

---

## 5. Data Structures

### Project
```rust
struct Project {
    name: String,
    sample_rate: u32,
    bpm: f64,
    time_signature: (u8, u8), // (numerator, denominator)
    tracks: Vec<Track>,
    master_bus: MasterBus,
    markers: Vec<Marker>,
}
```

### Track
```rust
struct Track {
    id: Uuid,
    name: String,
    color: Color,
    volume: f32,        // 0.0 to 1.0
    pan: f32,           // -1.0 to 1.0
    mute: bool,
    solo: bool,
    clips: Vec<AudioClip>,
    effects_chain: Vec<EffectInstance>,
    automation: HashMap<ParameterId, AutomationLane>,
}
```

### AudioClip
```rust
struct AudioClip {
    id: Uuid,
    source: AudioSource,         // File reference or internal
    position: TimePosition,      // Start time in project
    offset: TimePosition,        // Start position within source
    length: TimeDuration,
    fade_in: TimeDuration,
    fade_out: TimeDuration,
    gain: f32,
    time_stretch: Option<StretchParams>,
    pitch_shift: Option<PitchShiftParams>,
    waveform_cache: WaveformData,
}
```

### Automation
```rust
struct AutomationLane {
    parameter_id: ParameterId,
    points: Vec<AutomationPoint>,
    // Points stored as (time, value) pairs
    // Interpolated based on curve type
}

enum AutomationPoint {
    Linear { time: f64, value: f32 },
    Bezier { time: f64, value: f32, control_in: (f64, f32), control_out: (f64, f32) },
    Step { time: f64, value: f32 },
}
```

### Effect Chain
```rust
struct EffectInstance {
    id: Uuid,
    effect_type: EffectType,
    parameters: HashMap<String, ParameterValue>,
    bypass: bool,
}

enum EffectType {
    Equalizer,
    Compressor,
    Reverb,
    Delay,
    Distortion,
    // ... more internal effects
}
```

---

## 6. Core Components

### 6.1 Transport

Functions:
- Play, Stop, Pause, Loop
- Seek (click on timeline, scrub)
- Time display (bars:beats:ticks, time)
- Metronome (optional click track)
- Tempo changes (optional for v1)

### 6.2 Track Manager

Functions:
- Add/remove tracks
- Reorder tracks (drag-drop)
- Track naming, coloring
- Track grouping (folders)

### 6.3 Mixer

Each channel strip:
- Volume fader (dB scale, -inf to +6)
- Pan knob (-100L to +100R)
- Mute/Solo buttons
- Insert effect slots (8 slots)
- Send knobs (2-4 sends)
- Peak meter (real-time, fast/slow fall)
- Input/Output selector

Master bus:
- Same as channel strip
- Limiter (optional)
- Master meter with peak hold

### 6.4 Effects (Internal)

**v1 Effects:**
| Effect | Parameters |
|--------|------------|
| EQ (4-band) | Frequency, Gain, Q per band |
| Compressor | Threshold, Ratio, Attack, Release, Makeup |
| Reverb | Room size, Damping, Wet/Dry |
| Delay | Time, Feedback, Mix |
| Gain | Input gain, Output gain |

**v2+ Effects:**
| Effect | Parameters |
|--------|------------|
| Noise Gate | Threshold, Attack, Release, Range |
| Limiter | Threshold, Release |
| Distortion | Drive, Tone |
| Chorus | Rate, Depth |

### 6.5 Waveform Display

Implementation:
- Pre-calculate min/max peaks per pixel (offline)
- Cache waveform data on disk (like Ardour)
- Render using egui painter (waveform peaks as filled rects per pixel column)
- Support zoom (horizontal and vertical)
- Show playhead, loop region, clip boundaries

### 6.6 Automation

Implementation:
- Lane view below each track
- Click to add/move/delete points
- Curve types: Linear, Bezier, Step
- Parameter assignment (any effect parameter)
- Read/Write automation modes
- Automation recording from UI manipulation

### 6.7 Time-Stretch & Pitch-Shift

Using `timestretch` crate:
- Apply per-clip (non-real-time processing)
- Quality presets: fast, normal, high
- Beat-matching (optional)
- Preview before commit

---

## 7. Development Phases

### Phase 1: Foundation (Weeks 1-4)

**Goal:** Basic audio playback through mixer

| Task | Description |
|------|-------------|
| 1.1 | Project scaffolding with egui + cpal |
| 1.2 | Audio thread setup with callback |
| 1.3 | Load and play audio file (single track) |
| 1.4 | Basic transport (play/stop) |
| 1.5 | Volume/pan control on output |
| 1.6 | Multi-track playback |
| 1.7 | Mixdown to stereo |

**Milestone:** Can play multiple audio tracks through mixer with volume/pan

### Phase 2: Timeline & Clips (Weeks 5-8)

**Goal:** Visual timeline with audio clips

| Task | Description |
|------|-------------|
| 2.1 | Timeline view component |
| 2.2 | Clip rendering (rectangles with waveform preview) |
| 2.3 | Clip movement on timeline |
| 2.4 | Clip trimming (start/end) |
| 2.5 | Import audio files (drag-drop or menu) |
| 2.6 | Waveform generation (background task) |
| 2.7 | Playhead display and navigation |
| 2.8 | Zoom (horizontal, vertical) |

**Milestone:** Can import, arrange, and play audio clips on timeline

### Phase 3: Effects (Weeks 9-12)

**Goal:** Effect processing chain

| Task | Description |
|------|-------------|
| 3.1 | Effect base architecture |
| 3.2 | Effect parameter system |
| 3.3 | EQ effect implementation |
| 3.4 | Compressor effect implementation |
| 3.5 | Reverb effect implementation |
| 3.6 | Delay effect implementation |
| 3.7 | Effect bypass and ordering |
| 3.8 | Effect UI (knobs, graphs) |

**Milestone:** Can add effects to tracks with real-time processing

### Phase 4: Automation (Weeks 13-16)

**Goal:** Per-parameter automation

| Task | Description |
|------|-------------|
| 4.1 | Automation data model |
| 4.2 | Automation lane UI |
| 4.3 | Draw automation points |
| 4.4 | Point manipulation (drag, delete) |
| 4.5 | Curve interpolation (linear, bezier) |
| 4.6 | Automation playback |
| 4.7 | Record automation (from UI changes) |
| 4.8 | Automation lanes per track |

**Milestone:** Can write and playback automation for any parameter

### Phase 5: Time-Stretch & Pitch (Weeks 17-20)

**Goal:** Non-real-time time/pitch manipulation

| Task | Description |
|------|-------------|
| 5.1 | Integrate `timestretch` crate |
| 5.2 | Time-stretch UI (stretch factor) |
| 5.3 | Pitch-shift UI (semitones) |
| 5.4 | Offline processing with progress |
| 5.5 | Quality presets |
| 5.6 | Preview before apply |
| 5.7 | Preserve original (non-destructive) |

**Milestone:** Can time-stretch and pitch-shift audio clips

### Phase 6: Project I/O (Weeks 21-24)

**Goal:** Save and load projects

| Task | Description |
|------|-------------|
| 6.1 | Project file format (RON) |
| 6.2 | Serialize project state |
| 6.3 | Serialize clips (reference audio files) |
| 6.4 | Serialize automation |
| 6.5 | Serialize effect parameters |
| 6.6 | Recent projects list |
| 6.7 | Project recovery (auto-save) |

**Milestone:** Can save and reopen complete projects

### Phase 7: Polish (Weeks 25-28)

**Goal:** User experience improvements

| Task | Description |
|------|-------------|
| 7.1 | Keyboard shortcuts |
| 7.2 | Undo/Redo system |
| 7.3 | Preferences (audio device, buffer size) |
| 7.4 | Theme (dark mode default) |
| 7.5 | Performance optimization |
| 7.6 | Testing and bug fixes |

**Milestone:** Usable beta release

---

## 8. Key Implementation Challenges

### 8.1 Real-Time Safety

**Challenge:** Audio callback must not allocate memory or use locks.

**Solution:**
- Pre-allocate all buffers at project load time
- Use atomic operations for parameter changes
- Double-buffer UI <-> audio communication
- No `Vec` or `Box` in audio path

### 8.2 Waveform Rendering Performance

**Challenge:** Rendering large audio files at multiple zoom levels.

**Solution:**
- Pre-compute multi-resolution peaks (like Ardour)
- Cache to disk, load on demand
- Use SIMD for peak calculation
- Render visible portion only

### 8.3 Automation Interpolation

**Challenge:** Smooth automation playback at sample-accurate resolution.

**Solution:**
- Compute automation values at buffer boundaries
- Linear interpolation within buffer
- Bezier curves use lookup table for performance

### 8.4 Undo/Redo System

**Challenge:** Complex state changes with many components.

**Solution:**
- Command pattern with serialization
- Coalesce rapid changes (slider drags)
- Limit history depth (memory management)

---

## 9. File Structure (Proposed)

```
hdaw/
├── Cargo.toml
├── src/
│   ├── main.rs                 # Entry point
│   ├── app.rs                  # Application state
│   ├── audio/
│   │   ├── engine.rs           # Audio processing graph
│   │   ├── transport.rs        # Transport control
│   │   ├── mixer.rs           # Mixing logic
│   │   ├── effects/            # Effect implementations
│   │   │   ├── mod.rs
│   │   │   ├── eq.rs
│   │   │   ├── compressor.rs
│   │   │   ├── reverb.rs
│   │   │   └── delay.rs
│   │   └── buffer.rs           # Audio buffer management
│   ├── project/
│   │   ├── mod.rs              # Project model
│   │   ├── track.rs            # Track model
│   │   ├── clip.rs             # Audio clip model
│   │   ├── automation.rs       # Automation model
│   │   └── io.rs               # Save/load
│   ├── ui/
│   │   ├── app_ui.rs           # Panel layout (tiled: right, central, bottom, status)
│   │   ├── toolbar.rs          # Menu bar + transport controls
│   │   ├── timeline/           # Arrangement view (clips, ruler, automation)
│   │   ├── piano_roll.rs       # MIDI grid editor
│   │   ├── right_panel.rs      # Browser / Clip Info / FX Detail
│   │   ├── bottom_panel.rs     # Mixer / Sends / FX Chain
│   │   ├── preferences.rs      # Preferences dialog + state
│   │   ├── effect_editor/      # FX chain + parameter UI
│   │   ├── audio_pool.rs       # Imported audio pool
│   │   └── panels.rs           # Floating panel registry
│   └── utils/
│       ├── waveform.rs         # Waveform generation
│       └── timestretch.rs      # Time-stretch wrapper
└── tests/
    └── audio_tests.rs          # Audio processing tests
    └── midi_pipeline_test.rs   # MIDI→CLAP→audio integration tests
```

---

## 10. Dependencies (Summary)

```toml
[dependencies]
egui = "0.30"
eframe = "0.30"
cpal = "0.15"
dasp = "0.11"
serde = { version = "1.0", features = ["derive"] }
ron = "0.8"
tracing = "0.1"
tracing-subscriber = "0.3"
uuid = { version = "1.0", features = ["v4", "serde"] }

# Audio file handling
hound = "3.5"                 # WAV loading

# CLAP plugin hosting
clack-host = "0.1"
clack-extensions = "0.1"
```

---

## 11. Implementation Notes

### Build Setup for Windows

1. Install Rust: https://rustup.rs/
2. For WASAPI audio: No additional setup required (cpal handles it)
3. Install Visual Studio Build Tools if needed for C++ interop

### First Steps

1. Initialize new Rust project with cargo
2. Add egui and cpal dependencies
3. Verify egui window displays with audio callback
4. Confirm audio device enumeration works
5. Build minimal audio playback prototype

### Architecture Principles

1. **Audio thread is sacred** - Never block it, never allocate
2. **UI is immediate mode** - egui renders every frame from state
3. **Everything is a parameter** - Track volume, effect knobs, transport position
4. **Non-destructive editing** - Clips reference audio files, don't modify them
5. **State is serializable** - Entire project can be saved as RON file