# GUI & Audio Engine Performance Improvements

## Summary

Three independent fixes addressing progressive audio degradation, GUI memory leaks, and undo memory bloat.

## Fix 1: Non-blocking CLAP Parameter Updates from Audio Thread

### Problem
`evaluate_effect_params` → `EffectInstance::set_parameter` → `lock_clap_mut()` → `adapter.lock()` is a **blocking** `Mutex::lock()` called from the audio thread. If the UI thread holds the lock (e.g., slider drag), the audio thread stalls, causing audio dropouts.

### Design
Add `try_set_parameter()` to `ClapEffectAdapter` and `EffectInstance` that uses `pending_params.try_lock()` (non-blocking) instead of `pending_params.lock()` (blocking). If `try_lock()` fails (UI holds the lock), the automation update for this callback is dropped — the next callback retries (~23ms later, imperceptible). The CLAP plugin still receives `ParamValueEvent`s for the updates that succeed.

### Files Changed

| File | Change |
|------|--------|
| `src/audio/clap_effect.rs` | Added `try_set_parameter(&self, id, value)` using `pending_params.try_lock()` |
| `src/audio/effects/dsp_effect.rs` | Added `try_set_parameter()` to `EffectInstance` — non-blocking for CLAP, delegates to existing path for built-ins |
| `src/audio/automation_proc.rs` | `evaluate_effect_params` calls `inst.try_set_parameter()` instead of `inst.set_parameter()` |

## Fix 2: Thumbnail Cache Eviction on Clip Deletion

### Problem
`waveform_cache` and `midi_thumb_cache` (`HashMap<Uuid, TextureHandle>`) grow forever. GPU textures leak when clips are deleted.

### Design
Hook into `remove_selected_clip()` and `delete_track()` to remove cache entries by UUID.

### Files Changed

| File | Change |
|------|--------|
| `src/app/commands.rs` | In `remove_selected_clip()`, remove clip UUID from both caches after clip removal |
| `src/app/commands.rs` | In `delete_track()`, iterate track clips and remove each from both caches before cleanup |

## Fix 3: Strip Audio Buffer from Undo

### Problem
`UndoCommand::DeleteClip` stores `ClipKind::Audio(AudioClip)` including the full `AudioBuffer`. A 5-minute 44.1kHz stereo file is ~100MB. With `MAX_UNDO = 128`, worst case is 12.8GB. The buffer also lives in the audio pool entry — double storage.

### Design
Strip `buffer = None` from the AudioClip before pushing to undo. On undo `apply_undo`, if `buffer` is `None`, look up the pool entry by clip UUID to reconstruct the engine `ClipHandle`.

### Files Changed

| File | Change |
|------|--------|
| `src/app/commands.rs` | In `remove_selected_clip()`, strip `buffer = None` from `AudioClip` before pushing undo command |
| `src/app/undo/commands.rs` | In `apply_undo` for `DeleteClip`, when `buffer` is `None`, find pool entry by UUID and use its buffer for engine `ClipHandle` creation |

### Edge Cases
- Pool entry also cleared: `ClipHandle::new()` with empty `Vec<f32>` — clip produces silence, no crash
- Cache entries regenerate on next render frame via `or_insert_with`
- Undo/redo cycles work correctly across all three fixes
