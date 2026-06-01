# Effect Parameter Automation — Design Spec

## Overview
Extend HDAW's automation system (currently only Track Volume and Track Pan) to support automating any built-in or CLAP effect parameter. Users arm a parameter for automation via the effect editor, which creates an `AutomationLane` on the track. The lane is drawn, edited, and evaluated using the same interpolation engine as Volume/Pan.

## Data Model

### `AutomationLane` (project/automation.rs)

Add an optional effect instance reference:

```rust
pub struct AutomationLane {
    pub param_id: u32,
    pub param_name: String,
    pub points: Vec<AutomationPoint>,
    #[serde(skip)]
    pub dirty: bool,
    #[serde(default)]
    pub effect_instance_id: Option<Uuid>,
}
```

- `effect_instance_id = None` → track-level parameter (Volume, Pan) — existing lanes
- `effect_instance_id = Some(uuid)` → an effect parameter, where `uuid` matches an `EffectInstance.id` in the track's FX chain
- `param_id` for effect parameters is the index within that effect (0, 1, 2…) as returned by `ParameterInfo.id`

The new field is `#[serde(default)]`, so project files saved before this change load with `None` — fully backward-compatible.

## UI — Effect Editor

### Parameter "A" (Automate) Toggle

In `effect_editor/mod.rs`, the parameter slider section (currently lines 325–332) gains a small **A** button next to each slider:

```
[Slider: Gain ━━━━━━━━━○━━━━] [A]
[Slider: Threshold ━━━○━━━━] [A]
```

Click behavior:
- **No lane exists** → create an `AutomationLane` with `effect_instance_id = inst.id`, `param_id = p.id`, `param_name = p.name`, empty `points` vec. Add to both the engine `TrackHandle.automation_lanes` and the project `Track.automation_lanes`.
- **Lane exists** → remove the lane (including its points) from both models.
- The button is highlighted (e.g., colored/accented) when the lane exists.

### Lane Color Assignment

Each effect-param lane gets a color derived from its `effect_instance_id` via a simple hash:

```rust
fn lane_color(eid: Uuid) -> Color32 {
    let palette = [
        RGB(0xE9, 0x1E, 0x50), // red
        RGB(0x00, 0x96, 0xD6), // blue
        RGB(0x9C, 0x27, 0xB0), // purple
        RGB(0xFF, 0x98, 0x00), // orange
        RGB(0x4C, 0xAF, 0x50), // green
    ];
    palette[(eid.as_u128() as usize) % palette.len()]
}
```

### Effect Removal Cleans Up Lanes

In `remove_effect()` in `effect_editor/mod.rs` (and any other `remove_effect` path such as `commands.rs`), after removing the effect, iterate the track's automation lanes and remove any whose `effect_instance_id` matches the removed effect's `id`. This must be done on both the engine handle and the project track.

## UI — Timeline Automation Display

### Lane Drawing

`automation.rs::draw()` already iterates all lanes and uses a `match param_id` to pick color and Y-mapping. It falls through to a default gray for unknown IDs. The Y-mapping for generic parameters uses:

```rust
_ => bottom - value * height, // 0.0 at bottom, 1.0 at top
```

The `_` branch already works correctly for effect parameters — values are always 0.0–1.0 normalized. The only change needed is to use the new per-lane color instead of the hard-coded gray. We'll change the `draw()` function to accept or compute the color per lane rather than by param_id.

### Interaction (No Changes Needed)

`handle_automation_interaction()` in `auto_interaction.rs` works generically:
- `find_point()` / `find_segment()` use lane-relative coordinates
- `add_point_to_lane()` / `remove_point()` work on any lane
- `param_value_from_y()` has a `_ =>` branch for generic 0.0–1.0 mapping
- `AutoDragState` stores lane_index and works for any lane type
- `sync_automation_to_project()` is lane-index based and already correct

## Audio Processing

### Evaluation Strategy (Buffer-Granularity)

In `process_track()`, after the existing Volume/Pan automation evaluation (which is done once per buffer at position `pos` and applies to the entire buffer), add a new loop that evaluates effect-parameter lanes:

```rust
// Evaluate effect parameter automation lanes
// Use lane.get_value_at() directly (not param_id search) because
// the same param_id value may appear in different effects
for lane in &handle.automation_lanes {
    if let Some(eid) = lane.effect_instance_id {
        // Find the effect instance by UUID
        if let Some(inst) = handle.fx_chain.iter_mut().find(|e| e.id == eid) {
            let val = lane.get_value_at(pos_frames);
            if !val.is_nan() {
                inst.set_parameter(lane.param_id, val);
            }
        }
    }
}
```

This runs before the FX chain loop (before line 188 in process.rs). The value is evaluated at the buffer start position. For parameters that change mid-buffer, the evaluation happens once and the effect uses that value for the entire buffer — standard DAW practice for non-sample-accurate automation.

The `get_value_at` helper reuses the existing `AutomationLane::get_value_at()` logic. We can refactor the existing `automation_value()` helper to be callable for any param_id (it already is).

### Thread Safety

- `set_parameter()` already writes to `AtomicU32` on built-in effects (via `ParameterValue`) and uses `pending_params` + `try_lock` on CLAP effects
- The automation lane evaluation happens inside the engine mutex lock in `process_track()`, which is called from the audio callback via `mix_tracks()`
- No new locks or atomics needed

## Sync

### `sync_engine_to_project()` (project_io.rs)

Already iterates all `AutomationLane` objects and clones their points. The `effect_instance_id` field is `#[serde(default)]` and serialized automatically since tracks are serialized via `ron`. No changes needed.

### `sync_automation_to_project()` (auto_interaction.rs)

Already handles all lanes by index. No changes needed.

## Undo

Lane creation/deletion (the "A" button toggle) does NOT get undo support in v1. It is a lightweight operation that syncs both models immediately. The user is not expected to accidentally trigger it.

Existing undo for point operations (`AutomationAddPoint`, `AutomationRemovePoint`, `AutomationMovePoint`) continues to work for effect-param lanes since they use lane indices.

## Files Changed

| File | Change |
|------|--------|
| `project/automation.rs` | Add `effect_instance_id: Option<Uuid>` to `AutomationLane` |
| `audio/process.rs` | Add effect-param automation evaluation loop before FX chain |
| `ui/timeline/automation.rs` | Use per-lane color instead of param_id-based color for effect lanes |
| `ui/effect_editor/mod.rs` | Add "A" toggle per parameter slider; clean up lanes on effect removal |
| `app/commands.rs` | Clean up automation lanes in `remove_effect` path if called from there |

## What's NOT in v1

- No latch/touch/read/write automation modes — always read (lane overrides manual slider)
- No undo for lane creation/deletion
- No automation lane selector in timeline (users see all lanes overlaid)
- No effect parameter display in the automation lane area (lane name is shown via param_name in the tooltip or future info area)
- No sample-accurate automation (buffer-granularity is sufficient)
