# HDAW — Holofonic Digital Audio Workstation

A lightweight, native digital audio workstation built in Rust with real-time audio processing and an immediate-mode GUI.

## Features

- **Multi-track timeline** with waveform display, clip dragging, and trim editing
- **Audio pool** — import WAV files, drag clips onto tracks
- **5 built-in effects** — Gain, EQ (3-band parametric), Delay, Reverb, Compressor
- **Automation** — per-track volume/pan automation lanes with point editing
- **Mixer panel** — per-track volume/pan/mute/solo with VU meters
- **Transport** — play, pause, stop, loop region with draggable handles
- **Undo/redo** — command-based undo for clips, automation, effects, mute/solo
- **Project save/load** — RON-based project files with file references (not embedded audio)
- **Preferences dialog** — audio device, sample rate, buffer size, UI defaults, persisted to disk
- **Markers** — add markers at the playhead (M key)
- **Keyboard shortcuts** — Space (play/pause), Ctrl+S (save), Ctrl+Z (undo), etc.
- **Hot-plug resilient** — auto-rebuilds audio stream on device change
- **Real-time safe** — atomics for parameters, thread-local scratch buffers, no allocations in audio callback

## Quick Start

```bash
# Build and run
cargo run --release

# Run tests
cargo test
```

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Space` | Play / Pause |
| `Shift+Space` | Play / Pause (alt) |
| `Ctrl+S` | Save project |
| `Ctrl+Shift+S` | Save As |
| `Ctrl+O` | Open project |
| `Ctrl+N` | New project |
| `Ctrl+I` | Import audio |
| `Ctrl+Z` | Undo |
| `Ctrl+Shift+Z` | Redo |
| `Ctrl+,` | Preferences |
| `Delete` / `Backspace` | Delete selected clip |
| `L` | Toggle loop |
| `M` | Add marker at playhead |
| `Home` | Seek to start |
| `End` | Seek to end |
| `F2` | Toggle FX panel |

### Mouse Controls

| Input | Action |
|-------|--------|
| Left-click timeline lane | Seek playhead |
| Left-click clip | Select clip |
| Drag clip center | Move clip |
| Drag clip left/right edge | Trim clip |
| Middle-drag | Scroll timeline |
| Scroll wheel | Zoom timeline |
| Left-click automation lane | Add automation point |
| Drag automation point | Move point |
| Right-click automation point | Remove point |
| Left-click loop handle (ruler) | Drag loop region |

## Architecture

See [AGENTS.md](AGENTS.md) for the full architecture guide. Key points:

- **Dual-model sync** — serializable Project model + real-time Engine model, kept in sync manually
- **Real-time audio** — `cpal` callback, atomics for cross-thread params, thread-local scratch buffers
- **Immediate-mode UI** — `egui`/`eframe` 0.30, all state owned by `HdawApp`
- **File format** — RON serialization, audio files referenced by path (not embedded)

## Dependencies

| Crate | Purpose |
|-------|---------|
| egui/eframe 0.30 | UI framework |
| cpal 0.15 | Cross-platform audio I/O |
| dasp 0.11 | Audio types and DSP primitives |
| hound 3.5 | WAV file reading |
| egui_file_dialog 0.8 | File open/save dialogs |
| ron 0.8 | Project file serialization |
| serde 1.0 | Derive macros for serialization |
| uuid 1 | Unique clip/track IDs |
| tracing 0.1 | Structured logging |

## Project Structure

```
src/
  app/            Application state, commands, undo, I/O, preferences
  audio/          Engine, stream, transport, mixer, effects
  dsp/            Shared DSP math (biquad filters)
  project/        Data model (tracks, clips, automation, markers, pool)
  ui/             Toolbar, timeline, mixer, effect editor, preferences, audio pool
  utils/          Waveform peak extraction
  main.rs         Entry point
tests/            Integration tests
```
