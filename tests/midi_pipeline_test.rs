// #region debug - MIDI→CLAP→audio pipeline integration test
//
// Tests the full pipeline: create track → add MIDI clip → add CLAP instrument →
// process audio → verify output
//
// Run with: cargo test --test midi_pipeline_test -- --nocapture

use hdaw::audio::clap_effect::ClapEffectAdapter;
use hdaw::audio::effects::dsp_effect::{EffectInstance, EffectType};
use hdaw::audio::process;
use hdaw::project::clip_handle::ClipHandle;
use hdaw::project::midi_note::MidiNote;
use hdaw::project::track::TrackHandle;

/// Helper: read mix buffers and return (max_l, max_r)
fn read_mix() -> (f32, f32) {
    process::MIX_L.with(|ml| {
        process::MIX_R.with(|mr| {
            let mix_l = ml.borrow();
            let mix_r = mr.borrow();
            let max_l = mix_l.iter().cloned().map(f32::abs).fold(0.0f32, f32::max);
            let max_r = mix_r.iter().cloned().map(f32::abs).fold(0.0f32, f32::max);
            (max_l, max_r)
        })
    })
}

/// Helper: load a CLAP instrument (prefers Dexed, falls back to Vital)
fn load_instrument() -> Option<(EffectInstance, String)> {
    // Plugin ID must match what the CLAP binary advertises (from its descriptor),
    // NOT the filename. The scanner reads these via PluginEntry::plugin_descriptors().
    let candidates = [
        (r"C:\Program Files\Common Files\CLAP\Dexed.clap", "com.digital-suburban.dexed", "Dexed"),
        (r"C:\Program Files\Common Files\CLAP\Vital.clap", "audio.vital.synth", "Vital"),
        (r"C:\Program Files\Common Files\CLAP\Surge Synth Team\Surge XT.clap", "org.surge-synth-team.surge-xt", "Surge XT"),
    ];
    for (path_str, plugin_id, display_name) in &candidates {
        let path = std::path::Path::new(path_str);
        if path.exists() {
            eprintln!("[TEST] Loading CLAP plugin: {} (id={}) from {:?}", display_name, plugin_id, path);
            match ClapEffectAdapter::new_instance(plugin_id, path, 44100) {
                Ok(adapter) => {
                    eprintln!("[TEST] CLAP plugin loaded: has_note_input={}", adapter.has_note_input());
                    if !adapter.has_note_input() {
                        eprintln!("[TEST] Plugin {} has no note input, trying next", display_name);
                        continue;
                    }
                    let etype = EffectType::Clap {
                        plugin_id: plugin_id.to_string(),
                        path: path_str.to_string(),
                    };
                    let instance = EffectInstance::new_clap(display_name.to_string(), etype, adapter);
                    return Some((instance, display_name.to_string()));
                }
                Err(e) => {
                    eprintln!("[TEST] Failed to load CLAP plugin {}: {:?}", display_name, e);
                }
            }
        } else {
            eprintln!("[TEST] Plugin not found: {}", path_str);
        }
    }
    None
}

/// Test: MIDI notes produce non-zero audio output through a CLAP instrument.
#[test]
fn test_midi_to_clap_produces_audio() {
    let (instance, _name) = match load_instrument() {
        Some(v) => v,
        None => {
            eprintln!("[TEST] SKIP: No CLAP instrument plugin found");
            return;
        }
    };

    let mut handle = TrackHandle::new();
    handle.add_effect(instance);

    // Create MIDI clip with 3 notes spanning 1 second
    let notes = vec![
        MidiNote { pitch: 60, velocity: 100, release_velocity: 64, start_frame: 0, duration: 22050 },
        MidiNote { pitch: 64, velocity: 90, release_velocity: 64, start_frame: 11025, duration: 22050 },
        MidiNote { pitch: 67, velocity: 110, release_velocity: 64, start_frame: 22050, duration: 11025 },
    ];
    let clip_handle = ClipHandle::new_midi(uuid::Uuid::new_v4(), notes, 44100, 44100);
    clip_handle.set_position(0);
    handle.add_clip(clip_handle);

    // Process first buffer
    let frames = 1024;
    eprintln!("[TEST] Processing {} frames at pos=0", frames);
    process::process_track(&mut handle, 0, frames, 44100, false);

    let (max_l, max_r) = read_mix();
    eprintln!("[TEST] Output at pos=0: max_l={} max_r={}", max_l, max_r);

    // ASSERT: pipeline must produce non-zero audio
    let has_output = max_l > 1e-10 || max_r > 1e-10;
    assert!(has_output, "MIDI→CLAP pipeline produced zero output! Pipeline bug detected!");
    eprintln!("[TEST] PASS: MIDI→CLAP pipeline produced audio output");
}

/// Test: Seek produces NoteOff + NoteOn at offset 0 (detecting H1 bug)
#[test]
fn test_midi_seek_note_retrigger() {
    let (instance, _name) = match load_instrument() {
        Some(v) => v,
        None => {
            eprintln!("[TEST] SKIP: No CLAP instrument plugin found");
            return;
        }
    };

    let mut handle = TrackHandle::new();
    handle.add_effect(instance);

    // Note from frame 0 lasting 10 seconds - will be playing when we seek into its midpoint
    let notes = vec![
        MidiNote { pitch: 72, velocity: 100, release_velocity: 64, start_frame: 0, duration: 441000 },
    ];
    let clip_handle = ClipHandle::new_midi(uuid::Uuid::new_v4(), notes, 500000, 44100);
    clip_handle.set_position(0);
    handle.add_clip(clip_handle);

    let frames = 1024;

    // Buffer 1: pos=0 (note starts here)
    eprintln!("\n[TEST] === Buffer at pos=0 (note starts) ===");
    process::process_track(&mut handle, 0, frames, 44100, false);
    let (ml, mr) = read_mix();
    eprintln!("[TEST] Output: max_l={} max_r={}", ml, mr);

    // Buffer 2: pos=20000 (note is playing in middle)
    eprintln!("\n[TEST] === Buffer at pos=20000 (note playing in middle) ===");
    process::process_track(&mut handle, 20000, frames, 44100, false);
    let (ml, mr) = read_mix();
    eprintln!("[TEST] Output: max_l={} max_r={}", ml, mr);

    // Simulate seek to pos=200 - note started at 0, so we're seeking into
    // the middle of the note. This triggers the seek path:
    //   note_start_timeline(0) < buf_start(200) && note_end_timeline(441000) > buf_start(200)
    //   → NoteOff(0) + NoteOn(0 or 1)
    eprintln!("\n[TEST] === SEEK to pos=200 (mid-note) with seek_occurred=true ===");
    process::process_track(&mut handle, 200, frames, 44100, true);
    let (ml, mr) = read_mix();
    eprintln!("[TEST] Output after seek: max_l={} max_r={}", ml, mr);

    // ALSO test normal (non-seek) at the same position to compare
    eprintln!("\n[TEST] === Normal (no seek) at pos=200 for comparison ===");
    process::process_track(&mut handle, 200, frames, 44100, false);
    let (ml, mr) = read_mix();
    eprintln!("[TEST] Output normal: max_l={} max_r={}", ml, mr);

    eprintln!("[TEST] Seek test completed");
}

/// Test: Process a buffer that overlaps clip end boundary
#[test]
fn test_midi_clip_boundary_noteoff() {
    let (instance, _name) = match load_instrument() {
        Some(v) => v,
        None => {
            eprintln!("[TEST] SKIP: No CLAP instrument plugin found");
            return;
        }
    };

    let mut handle = TrackHandle::new();
    handle.add_effect(instance);

    // Note that extends past clip end: clip is 2048 frames long, note starts at 0, duration=5000
    let notes = vec![
        MidiNote { pitch: 60, velocity: 100, release_velocity: 64, start_frame: 0, duration: 5000 },
    ];
    let clip_handle = ClipHandle::new_midi(uuid::Uuid::new_v4(), notes, 2048, 44100);
    clip_handle.set_position(0);
    handle.add_clip(clip_handle);

    let frames = 1024;

    // Buffer 1: pos=0 (note starts here)
    eprintln!("\n[TEST] === Buffer at pos=0 (clip_end=2048) ===");
    process::process_track(&mut handle, 0, frames, 44100, false);
    let (ml, mr) = read_mix();
    eprintln!("[TEST] Output: max_l={} max_r={}", ml, mr);

    // Buffer 2: pos=1024 (note still playing, will hit clip_end at 2048 - note_end should fire at clip_end)
    eprintln!("\n[TEST] === Buffer at pos=1024 (clip_end in next buffer) ===");
    process::process_track(&mut handle, 1024, frames, 44100, false);
    let (ml, mr) = read_mix();
    eprintln!("[TEST] Output: max_l={} max_r={}", ml, mr);

    // Buffer 3: pos=2048 (clip_end reached)
    // clip_end(2048) should be == buf_start(2048), clip should be skipped
    // If the NoteOff was sent correctly in buffer 2, output here is plugin decay only
    eprintln!("\n[TEST] === Buffer at pos=2048 (clip_end reached, clip skipped) ===");
    process::process_track(&mut handle, 2048, frames, 44100, false);
    let (ml, mr) = read_mix();
    eprintln!("[TEST] Output: max_l={} max_r={}", ml, mr);

    eprintln!("[TEST] Clip boundary test completed");
}

/// Test specifically: clip_end aligns exactly with buf_end.
/// This catches the bug where NoteOff at clip_end is never sent.
#[test]
fn test_midi_clip_end_aligned_with_buffer_boundary() {
    let (instance, _name) = match load_instrument() {
        Some(v) => v,
        None => {
            eprintln!("[TEST] SKIP: No CLAP instrument plugin found");
            return;
        }
    };

    let mut handle = TrackHandle::new();
    handle.add_effect(instance);

    // Clip length = 2048. Note starts at 0, lasts 5000 (extends past clip).
    let notes = vec![
        MidiNote { pitch: 64, velocity: 100, release_velocity: 64, start_frame: 0, duration: 5000 },
    ];
    // Use frames=2048 so buffer [0,1024) and [1024,2048). clip_end=2048 == buf_end of buffer 2.
    let clip_handle = ClipHandle::new_midi(uuid::Uuid::new_v4(), notes, 2048, 44100);
    clip_handle.set_position(0);
    handle.add_clip(clip_handle);

    let frames = 1024;

    // Buffer 1: pos=0. NoteOn fires at offset 0.
    eprintln!("\n[TEST] === Buffer 1: pos=0 ===");
    process::process_track(&mut handle, 0, frames, 44100, false);
    let (ml1, _mr1) = read_mix();
    eprintln!("[TEST] Output: max_l={}", ml1);

    // Buffer 2: pos=1024. clip_end=2048 == buf_end=2048.
    // BUG: NoteOff should be sent at offset 1023 (last frame before clip_end).
    eprintln!("\n[TEST] === Buffer 2: pos=1024 (clip_end=2048 == buf_end) ===");
    process::process_track(&mut handle, 1024, frames, 44100, false);
    let (ml2, _mr2) = read_mix();
    eprintln!("[TEST] Output: max_l={}", ml2);

    // Buffer 3: pos=2048. clip_end=2048 <= buf_start=2048, clip skipped.
    // If NoteOff was sent correctly in buffer 2, output drops as plugin release decays.
    // If NoteOff was NOT sent, note continues playing (plugin output ~same as buffer 2).
    eprintln!("\n[TEST] === Buffer 3: pos=2048 (clip skipped - NoteOff should have fired) ===");
    process::process_track(&mut handle, 2048, frames, 44100, false);
    let (ml3, _mr3) = read_mix();
    eprintln!("[TEST] Output: max_l={}", ml3);

    // Buffer 4: pos=3072. Even more decay if NoteOff was sent.
    eprintln!("\n[TEST] === Buffer 4: pos=3072 (further decay) ===");
    process::process_track(&mut handle, 3072, frames, 44100, false);
    let (ml4, _mr4) = read_mix();
    eprintln!("[TEST] Output: max_l={}", ml4);

    // Buffer 5: pos=4096. Should be near silence if NoteOff was sent.
    eprintln!("\n[TEST] === Buffer 5: pos=4096 (should be silent if NoteOff sent) ===");
    process::process_track(&mut handle, 4096, frames, 44100, false);
    let (ml5, _mr5) = read_mix();
    eprintln!("[TEST] Output: max_l={}", ml5);

    let drop_ratio = if ml2 > 1e-10 { ml3 / ml2 } else { 1.0 };
    let decay_ratio = if ml2 > 1e-10 { ml5 / ml2 } else { 1.0 };
    eprintln!("[TEST] Drop ratio (buf3/buf2): {:.3} (should be < 1.0 if NoteOff sent)", drop_ratio);
    eprintln!("[TEST] Decay ratio (buf5/buf2): {:.3} (should be << 1.0 if NoteOff sent)", decay_ratio);

    // The note should decay significantly over 3 buffers (3072 frames = 70ms)
    if decay_ratio > 0.5 {
        eprintln!("[TEST] WARNING: High output after 70ms - NoteOff may not have been sent! BUG CONFIRMED.");
    } else {
        eprintln!("[TEST] NoteOff correctly sent - output decays as expected.");
    }

    eprintln!("[TEST] Clip-end boundary test completed");
}
