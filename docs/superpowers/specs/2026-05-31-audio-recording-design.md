# Audio Recording — Design Spec

## Overview
Add real-time audio recording to HDAW. The user can arm one or more tracks, press record, capture audio from the system input device, and have the recorded clip appear on the timeline — all without blocking the audio thread.

## Data Flow
```
CPAL Input Stream callback (audio thread)
  → sync_channel::try_send(Arc<Vec<f32>>)     [lock-free, bounded]
  → Recording worker thread::recv()            [dedicated thread, blocking]
  → hound::WavWriter::write_sample()          [disk I/O, never in audio thread]
  → On stop: load saved WAV → create ClipHandle + AudioClip → add to track + pool
```

## Components

### 1. Ring Buffer (`audio/record.rs`)
- `std::sync::mpsc::sync_channel::<RecordPacket>` with capacity 64
- `RecordPacket = Arc<Vec<f32>>` (interleaved f32 samples)
- `try_send()` from input callback (drops if full — indicates overload)
- `recv()` from worker thread (blocks until data available)

### 2. Input Stream (`audio/stream.rs`)
- `build_input_stream()` — separate CPAL input stream on default input device
- 512-sample buffer (configurable)
- Callback: captures interleaved f32 → pushes to channel
- Uses `thread_local!` buffer to avoid allocation in callback
- Started/stopped with recording (cold-start approach saves CPU when idle)

### 3. Recording Manager (`audio/record.rs`)
```
enum RecordingState {
    Idle,
    Recording {
        start_frame: u64,       // project position when record started
        file_path: PathBuf,     // full path to recording WAV
        worker: JoinHandle<()>, // recording thread handle
        sender: SyncSender<RecordPacket>,
        stop_flag: Arc<AtomicBool>,
    },
}
```
- Wrapped in `Mutex` on `AudioEngine`
- Accessible from UI for state queries and stop signaling

### 4. Recording Worker (`audio/record.rs`)
- Spawned when record starts:
  1. Open `hound::WavWriter` at `file_path` (32-bit float, stereo, engine SR)
  2. Loop: `recv()` from channel → write samples
  3. Check `stop_flag` periodically
  4. On stop: drain remaining channel samples, finalize WAV, return stats
- Path: `{project_dir}/Audio Recordings/{track_name}_{YYYYMMDD_HHMMSS}.wav`
- If no `project_dir`, use a sensible default (e.g., current working directory)

### 5. Engine Changes (`audio/engine.rs`)
```
AudioEngine {
    // New fields:
    recording: Arc<Mutex<RecordingState>>,
    recording_channels: AtomicU32,  // 2 for stereo
}
```
- `start_recording()`: creates worker, starts input stream, sets state
- `stop_recording()`: signals stop, joins worker, loads WAV, creates clips
- Called from `HdawApp` via `play_requested`/`stop_requested`-style flags

### 6. Track Arming (`project/track.rs`)
- `TrackHandle.armed: AtomicBool`
- Default: `false`
- Toggled from UI: `handle.armed.fetch_xor(true, Ordering::Release)`

### 7. Clip Creation
After recording stops:
1. Load the WAV via `hound::WavReader` → `AudioBuffer::from_interleaved`
2. Create `ClipHandle::new()` with recorded buffer
3. Create `ClipKind::Audio(AudioClip)` with source path set to recording file
4. Set clip position to `start_frame` (the project frame where recording began)
5. Add clip to each armed track (both engine and project models)
6. Add `PoolClip` entry to `Project.audio_pool`
7. Push undo command

### 8. UI Changes

#### Track Header (`ui/timeline/track_headers.rs`)
- Arm button per track: small red circular button
- Toggle arm state on click
- Visual feedback: red background when armed, gray otherwise

#### Toolbar (`ui/toolbar.rs`)
- Record button (●) added to transport group
- Red color, pulses when recording (alternating every 500ms via `ctx.request_repaint()`)
- Disabled when no tracks are armed

#### App-level (`app/mod.rs`)
- Add `record_requested: bool`, `recording: bool` to `HdawApp`
- Handle record button → `start_recording()` / `stop_recording()`
- Run `check_recording_completion()` in update loop

## State Machine

```
Idle ──arm track + press record──→ Recording
Recording ──press stop or space──→ Finalizing (non-blocking handoff)
Finalizing ──worker joined──→ Idle (clips created)
```

## Edge Cases

| Case | Behavior |
|------|----------|
| No tracks armed | Record button disabled, tooltip says "Arm a track first" |
| Channel full (input > output processing) | `try_send()` drops oldest samples. Visual indicator of recording overload in status bar. |
| Disk full | `hound::WavWriter::write_sample()` may fail. Error captured via `mpsc::Receiver` → `try_recv()` error path → shown in status bar. |
| No input device | `build_input_stream()` fails → show error, stay in Idle |
| Recording + playing | Input stream + output stream run concurrently. Works naturally. |
| Record on empty timeline | Clip created at frame 0 on the armed track. |
| Project not saved | Fall back to `std::env::current_dir()` + `Audio Recordings/` |
| Multiple armed tracks | Same audio recorded to each armed track (mono sum → each track). In the future: per-track input routing. |

## Scope (YAGNI)
- ✅ Immediate recording (no count-in bars)
- ✅ Mono mixdown to all armed tracks (same input)
- ✅ 32-bit float WAV output
- ❌ Punch-in/punch-out (deferred to Tier 2)
- ❌ Input monitoring (deferred)
- ❌ Multi-channel input selection (deferred)
- ❌ Latency compensation (deferred)
- ❌ Non-destructive recording (always creates new file)

## Files Changed
| File | Change |
|------|--------|
| `Cargo.toml` | (none — uses std::sync::mpsc + existing hound) |
| `audio/mod.rs` | Add `pub mod record;` |
| `audio/record.rs` | New file — ring buffer, worker, RecordingState, start/stop |
| `audio/stream.rs` | Add `build_input_stream()`, input callback |
| `audio/engine.rs` | Add recording fields, `start_recording()`, `stop_recording()` |
| `project/track.rs` | Add `armed: AtomicBool` to `TrackHandle` |
| `project/mod.rs` | (none — PoolClip and ClipKind already support this) |
| `app/mod.rs` | Add `record_requested`, `recording` flags, handle record |
| `ui/toolbar.rs` | Add record button + `record_clicked` action |
| `ui/timeline/track_headers.rs` | Add arm button per track |
| `ui/app_ui.rs` | Wire record action |

## Test Plan
1. Arm a track → record button enables
2. Press record → input stream starts → recording indicator shows
3. Speak/make noise → stop → clip appears on track with waveform
4. Play back → recorded audio is heard
5. No project path → recording saves to current dir
6. No input device → error shown, stays in Idle
7. Multiple armed tracks → clip on each track
