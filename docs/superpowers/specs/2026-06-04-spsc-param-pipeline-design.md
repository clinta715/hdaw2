# Lock-Free SPSC Parameter Pipeline

## Status: Accepted

## 1. Overview

Replace the current `pending_params: Mutex<Vec<(u32, f64)>>` + `try_lock()` approach with a **lock-free SPSC ring buffer** between the UI thread (producer) and audio thread (consumer).

- UI thread never blocks on a lock
- Audio thread never blocks — reads what it can, drops what it can't
- Zero lock contention in the parameter path
- Retains the 1024-entry cap with overflow warning

**Target:** Eliminate dropouts caused by GUI interactions clashing with audio thread on the `pending_params` mutex.

## 2. Data Structures

### Ring Buffer (Lock-Free SPSC)

```rust
struct ParamRingBuffer {
    entries: Vec<ParamChange>,      // power-of-2 size, e.g. 1024 or 2048
    write_idx: AtomicU64,           // UI thread writes here
    read_idx: AtomicU64,            // audio thread reads here
    cap: usize,                     // mask for wrap detection
}

struct ParamChange {
    param_id: u32,
    value: f64,
}
```

- Power-of-2 size enables fast modulo via bitmask: `idx & (size - 1)`
- `write_idx` only modified by UI; `read_idx` only modified by audio
- No locks needed — each index is independently atomic
- Cap: 1024 entries. If queue full, drop and warn once per session.

### Overflow Tracking

```rust
static OVERFLOW_WARNED: AtomicBool = AtomicBool::new(false);
```

One-time warning per session when overflow occurs. Resets on engine init (new session, load project, etc.).

## 3. Producer Side (UI Thread)

### `ParamRingBuffer::push(param_id, value) -> bool`

1. Read `write_idx` and `read_idx` (both `AtomicU64`, `Ordering::Acquire`)
2. If `(write_idx - read_idx) >= cap` → queue full → log warning once → return false
3. Write `ParamChange` at `write_idx & mask`
4. `write_idx.fetch_add(1, Ordering::Release)` — publish to consumer
5. Return true

### Replacement of Existing `try_set_parameter`

- `ClapEffectAdapter::set_parameter()` no longer locks or pushes to `pending_params Mutex`
- Calls `ParamRingBuffer::push()` directly
- All built-in effects (`set_parameter`) also route through the ring buffer

### Lifetime

Owned by `AudioEngine`, passed to `Stream` on build. On stream rebuild (changing sample rate or buffer size), the queue is **discarded** — automation in flight is lost, but the next buffer picks up from the current timeline position.

## 4. Consumer Side (Audio Thread)

### `ParamRingBuffer::drain(&mut self, count: usize) -> Vec<ParamChange>`

1. Read `read_idx` (atomic)
2. Compute available: `write_idx()` - read_idx (via `Ordering::Acquire` on both)
3. `min(available, count)` entries to drain
4. For each: read entry at `read_idx & mask`, increment `read_idx`
5. Return drained entries (reused thread-local scratch `Vec<ParamChange>`)

### Integration in `process_track()`

Drain ring buffer **once at top of track processing**, apply params directly via `set_parameter_unchecked()` — bypassing the queue entirely on the audio thread side. This avoids per-effect queue overhead.

```
process_track()
  → drain ring buffer → apply directly to effects (no queue, no lock)
  → automation evaluation → also direct apply
  → FX chain processing
```

## 5. Code Changes

### `audio/param_ring.rs` (new file)

`ParamRingBuffer` struct with `push()` and `drain()` methods. `ParamChange` struct.

### `audio/clap_effect.rs`

- Remove `pending_params: Mutex<Vec<(u32, f64)>>` field
- Remove `PENDING_CAP` constant
- Add `ring_buffer: ParamRingBuffer` field to `ClapEffectAdapter`
- `set_parameter()` → `ring_buffer.push()` (UI path)
- `process_inner()` → drain ring buffer into `COMBINED_EVENTS` directly, NOT through `pending_params`
- Remove `try_lock()` on `pending_params` entirely

### `audio/automation_proc.rs`

- `evaluate_effect_params()` — drain ring buffer directly at start
- Apply params to effects via direct internal method (no lock, no queue)
- Can reuse or remove `pending_params` field on `ClapEffectAdapter`

### `audio/dsp_effect.rs`

- `EffectInstance::set_parameter()` — route through ring buffer for CLAP effects
- `BuiltIn` variant: still atomics, but audio thread reads ring buffer on each `process_track()` call and applies directly

### `audio/stream.rs`

- `mix_tracks()` receives `&ParamRingBuffer` reference
- `process_track()` drains and applies at top of each track

### `audio/engine.rs`

- `AudioEngine` owns the `ParamRingBuffer` instance
- Passes reference to `Stream::build()` and `Stream::rebuild()`
- On rebuild: move the ring buffer rather than recreate it

### `app_ui.rs` / parameter UI code

- All param writes go through `engine.set_parameter()` → ring buffer `push()`
- No locking on the UI side

## 6. Overflow Warning

Once per session warning when queue overflows:

```rust
use tracing::warn;

static OVERFLOW_WARNED: AtomicBool = AtomicBool::new(false);

fn push(...) {
    if queue_full {
        if !OVERFLOW_WARNED.swap(true, Ordering::Relaxed) {
            warn!("Parameter automation queue overflow — some changes dropped");
        }
        return false;
    }
}
```

Reset on engine init (new session, load project).

## 7. Testing Approach

### Unit Tests

- Ring buffer push/drain correctness (single-threaded)
- Multi-threaded stress: UI thread pushing + audio thread draining simultaneously
- Overflow behavior: fill queue, verify drop + warning

### Integration Tests

- Load project with automation
- Rapidly drag parameter slider while playing
- Monitor audio output for dropouts

### Regression

- Existing `tests/midi_pipeline_test.rs` still passes
- Existing automation tests still pass

## 8. Open Questions — RESOLVED

| Question | Resolution |
|----------|------------|
| Overflow behavior | (B) Log once-per-session warning, then silent drop |
| Queue survives hot-reload | No — discarded on stream rebuild |
| Audio thread apply | Drain once at top of `process_track()`, apply directly |
| Existing `try_lock` code | Fully replaced — no more `pending_params Mutex` |