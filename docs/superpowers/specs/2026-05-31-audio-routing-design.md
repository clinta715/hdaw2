# Audio Routing: Group Tracks & Send FX — Design Spec

## Overview
Add Cubase-style group tracks (accumulator busses with FX, no clips) and Ableton-style send/return tracks to HDAW. Every track can route to exactly one group (or master), and can send any amount of signal to any number of return tracks pre- or post-fader.

## Data Model

### TrackHandle (engine) additions
```rust
parent_group: Option<Uuid>   // None = route to master
is_group: bool                // accumulator bus, no clips, appears as bus channel
is_return: bool               // send return, no clips, appears as return channel
sends: Vec<SendSlot>
```

### Track (project model) additions
Same fields, serialized with `#[serde(default)]`.

### SendSlot
```rust
pub struct SendSlot {
    pub target_id: Uuid,   // return track UUID
    pub level: f32,        // 0.0 – 1.0
    pub pre_fader: bool,   // pre-fader (true) or post-fader (false)
}
```

Engine model wraps `level` in `Arc<AtomicU32>` for real-time safety.

### TrackUiState additions
```rust
pub parent_group: Option<Uuid>,
pub is_group: bool,
pub is_return: bool,
pub sends: Vec<Arc<AtomicU32>>,  // send levels (indexed by return track order)
pub collapsed: bool,             // timeline collapse state for group children
```

## Processing (Audio Thread)

`mix_tracks()` becomes a multi-pass pipeline using thread-local accumulators.

### Accumulators
```
thread_local! {
    ACCUM_L: RefCell<Vec<f32>>,    // master (existing SCRATCH_L)
    ACCUM_R: RefCell<Vec<f32>>,    // master (existing SCRATCH_R)
    GROUP_BUFS_L: RefCell<Vec<Option<Vec<f32>>>>,  // per-group, indexed by track index
    GROUP_BUFS_R: RefCell<Vec<Option<Vec<f32>>>>,
    RETURN_BUFS_L: RefCell<Vec<Option<Vec<f32>>>>,  // per-return, indexed by track index
    RETURN_BUFS_R: RefCell<Vec<Option<Vec<f32>>>>,
}
```

Per-chunk setup: resize all buffers to `frames` and zero, resize group/return buffer Vecs to `track_list.len()`, set to None.

### Pass 1 — Source Tracks
For each track where `!is_group && !is_return`:
1. `process_track(handle, track_l, track_r, ...)` into private per-track output buffers (reuse existing scratch pattern or add temp thread_locals)
2. If `parent_group.is_some()`: add track_l/r → `GROUP_BUFS_L[group_idx]` / `GROUP_BUFS_R[group_idx]`
3. Else: add track_l/r → master ACCUM_L / ACCUM_R
4. For each `SendSlot`:
   - Source signal = `track_l/r` if post-fader, or pre-fader signal (pre-volume/pan) if `pre_fader`
   - Scale by `send_level`
   - Add → `RETURN_BUFS_L[return_idx]` / `RETURN_BUFS_R[return_idx]`

### Pass 2 — Group Tracks (Topological Order)
Groups must be processed bottom-up: a group's children (which may be other groups) must be processed before the parent.

Algorithm: build adjacency from `parent_group` UUIDs, then Kahn's algorithm on the group subgraph. Since cycle detection runs on every route change, the graph is guaranteed acyclic.

Each group processed:
1. Take the group's accumulated buffer from Pass 1
2. Run the group's FX chain on it
3. If `parent_group.is_some()`: add → parent's GROUP_BUF (this parent will be processed later in this pass)
4. Else: add → master ACCUM_L / ACCUM_R
5. For each `SendSlot`: copy signal at send level → return's RETURN_BUF

### Pre-Fader Send Signal
`process_track` currently applies volume/pan inline (writes to the output buffer). To support pre-fader sends, `process_track` gains an optional output parameter `pre_fader_out: Option<(&mut [f32], &mut [f32])>`. When populated, the raw clip sum (before volume/pan multiplication) is written here alongside the normal post-fader output. This keeps the existing API unchanged for callers that don't need pre-fader signal.

### Pass 3 — Return Tracks
For each track where `is_return`:
1. Take the return's accumulated buffer from Pass 1 + Pass 2
2. Run the return's FX chain on it
3. Add → master ACCUM_L / ACCUM_R

### Pass 4 — Master Bus
Existing `master_bus.process(ACCUM_L, ACCUM_R)`, then add metronome.

```
NOTE on pre-fader sends: process_track currently applies volume/pan inline.
To get pre-fader signal, we need process_track to return (or write to a second buffer)
the signal before volume/pan is applied. This requires a small refactor:
process_track produces two outputs: pre_fader_l/r (raw clip sum) and post_fader_l/r
(after volume/pan). Normal routing uses post_fader, pre-fader sends use pre_fader.
```

## UI

### Mixer Panel
- Each channel strip shows a **"RTE"** dropdown listing all group tracks + "Master"
- An expandable **"Sends"** section per strip: one slider per return track (dB scale, 0.0–1.0) + pre-fader toggle button
- Group strips have no RTE dropdown (route determined by their own `parent_group`)
- Return strips have a muted header label "RETURN" and no Sends section (they can still have FX)

### Timeline
- Group tracks appear as headers with no waveform area (empty clip lane) and a collapsible arrow ▸/▾
- Clicking ▸/▾ on a group header hides/shows ALL descendant tracks (direct children + their children recursively) in the track list. This matches Cubase behavior where collapsing the top-level group hides everything inside.
- Collapse state stored in `TrackUiState.collapsed: bool`.
- Return tracks appear at the bottom of the track list (after all groups) with no clip lane or just a thin header.

### Track Menu
- "Add Group Track" → creates `TrackHandle { is_group: true }` at cursor position
- "Add Return Track" → creates `TrackHandle { is_return: true }` appended after all tracks

### Creation Flow
Groups and returns are regular tracks in the track list. The `is_group`/`is_return` flags change:
- Whether clips can be added (rejected in `add_clip()` for groups/returns)
- How they appear in UI (collapsible, no waveform lane)
- How they're processed (multi-pass accumulator vs single-pass)

## Serialization

`Track` struct additions:
```rust
pub parent_group: Option<Uuid>,  // #[serde(default)]
pub is_group: bool,              // #[serde(default)]
pub is_return: bool,             // #[serde(default)]
pub sends: Vec<SendSlot>,        // #[serde(default)]
```

`SendSlot`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendSlotExport {
    pub target_id: Uuid,
    pub level: f32,
    pub pre_fader: bool,
}
```

## Cycle Detection

On route change (setting `parent_group` on any track):
1. Start from the target group
2. DFS follow `parent_group` links
3. If we encounter the source track's UUID → reject the route change with error

This is a quick O(n) check run on the UI thread. It catches:
- Routing a group to itself
- Routing A→B where B→A (direct cycle)
- Routing A→B→C→A (multi-hop cycle)

## Thread Safety

All new hot-path routing state uses the existing atomic pattern:
- `SendSlot.send_level` → `Arc<AtomicU32>` (f32::bits)
- `TrackHandle.parent_group` → read-only after construction, changed via UI-mutex pattern
- Group/return accumulators → thread_local, only touched by audio callback

## Files Changed

| File | Change |
|------|--------|
| `project/track.rs` | Add `parent_group`, `is_group`, `is_return`, `sends` to TrackHandle + Track; add `SendSlot` |
| `project/clip.rs` | Reject adding clips to `is_group` tracks in `add_clip()` if called from UI |
| `project/mod.rs` | Export `SendSlot` |
| `audio/stream.rs` | Multi-pass `mix_tracks()` with group/return accumulators |
| `audio/process.rs` | `process_track` produces pre-fader output buffer for sends |
| `audio/engine.rs` | Add/remove group/return track helpers |
| `app/mod.rs` | `TrackUiState` additions, `add_group_track()`, `add_return_track()`, `set_track_parent()`, `set_send_level()`, cycle detection |
| `app/project_io.rs` | Serialization of routing fields |
| `app/undo/mod.rs` | No new undo variants needed (routing state is on the track, changes are direct) |
| `ui/mixer_panel.rs` | Route dropdown, sends section per strip |
| `ui/timeline/track_headers.rs` | Collapse arrow for groups, styling for returns |
| `ui/timeline/interaction.rs` | Collapse click handler |
| `ui/timeline/mod.rs` | Track layout respects collapse state, skips clip lanes for group/return |
| `ui/toolbar.rs` | "Add Group Track", "Add Return Track" menu items |
| `ui/app_ui.rs` | Wire new toolbar actions |
