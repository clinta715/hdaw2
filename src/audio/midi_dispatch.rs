use crate::audio::effects::dsp_effect::EffectKind;
use crate::project::track::TrackHandle;
use clack_host::events::event_types::{MidiEvent, NoteOffEvent, NoteOnEvent};
use clack_host::events::io::EventBuffer;
use clack_host::events::Pckn;
use std::cell::RefCell;
use std::sync::atomic::Ordering;

thread_local! {
    static MIDI_EVENTS: RefCell<EventBuffer> = RefCell::new(EventBuffer::with_capacity(512));
}

/// Scans all MIDI clips on the track, builds NoteOn/NoteOff events for the
/// current buffer window, and dispatches them to the first note-capable
/// instrument in the FX chain. Writes instrument output into `mix_l`/`mix_r`.
///
/// Returns the instrument index so the caller can skip it in the FX chain loop.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_midi(
    handle: &mut TrackHandle,
    pos: usize,
    frames: usize,
    sample_rate: u32,
    seek_occurred: bool,
    mix_l: &mut [f32],
    mix_r: &mut [f32],
    track_vol: f32,
) -> Option<usize> {
    let inst_idx = handle.fx_chain.iter().position(|e| e.has_note_input);
    let has_midi_clips = handle.clips.iter().any(|c| !c.midi_notes.is_empty());
    if has_midi_clips && inst_idx.is_none() {
        tracing::warn!(
            track_id = %handle.id,
            "track has MIDI clips but no instrument in FX chain"
        );
    }

    let ii = inst_idx?;
    if handle.fx_chain[ii].is_bypassed() {
        return Some(ii);
    }

    if let EffectKind::Clap(adapter) = &handle.fx_chain[ii].kind {
        if let Ok(mut a) = adapter.try_lock() {
            if seek_occurred {
                a.reset();
            }
            MIDI_EVENTS.with(|eb| {
                let mut buf = eb.borrow_mut();
                buf.clear();
                let buf_start = pos as u64;
                let buf_end = (pos + frames) as u64;

                for clip in &handle.clips {
                    if clip.midi_notes.is_empty() {
                        continue;
                    }
                    let clip_start = clip.position_frames.load(Ordering::Acquire);
                    let clip_off = clip.offset_frames.load(Ordering::Acquire);
                    let clip_len = clip.length_frames.load(Ordering::Acquire);
                    let clip_end = clip_start + clip_len.saturating_sub(clip_off);

                    if clip_start >= buf_end || clip_end <= buf_start {
                        continue;
                    }

                    for note in &clip.midi_notes {
                        let note_start_timeline =
                            (clip_start as i64 + note.start_frame as i64 - clip_off as i64).max(0)
                                as u64;
                        let note_end_timeline = note_start_timeline + note.duration;

                        let visible_note_start = note_start_timeline.max(clip_start);
                        let visible_note_end = note_end_timeline.min(clip_end);

                        if visible_note_start >= visible_note_end {
                            continue;
                        }

                        if seek_occurred
                            && note_start_timeline < buf_start
                            && note_end_timeline > buf_start
                        {
                            let pckn = Pckn::new(0u8, 0u8, note.pitch, 0u8);
                            buf.push(&NoteOffEvent::new(0, pckn, 0.0));
                            // NoteOn at offset 1 (not 0) to prevent unstable-sort reordering
                            // with the NoteOff at offset 0
                            buf.push(&NoteOnEvent::new(
                                1,
                                pckn,
                                note.velocity as f64 / 127.0,
                            ));
                        } else if note_start_timeline >= buf_start
                            && note_start_timeline < buf_end
                            && note_start_timeline >= clip_start
                        {
                                let offset = (note_start_timeline - buf_start) as u32;
                                if offset < frames as u32 {
                                    let pckn = Pckn::new(0u8, 0u8, note.pitch, 0u8);
                                    let vel = note.velocity as f64 / 127.0;
                                    buf.push(&NoteOnEvent::new(offset, pckn, vel));
                                }
                        }

                        if note_end_timeline >= buf_start && note_end_timeline < buf_end {
                            if note_end_timeline <= clip_end {
                                let offset = (note_end_timeline - buf_start) as u32;
                                if offset < frames as u32 {
                                    let pckn = Pckn::new(0u8, 0u8, note.pitch, 0u8);
                                    buf.push(&NoteOffEvent::new(offset, pckn, 0.0));
                                }
                            }
                        } else if note_end_timeline > clip_end
                            && clip_end >= buf_start
                            && clip_end <= buf_end
                        {
                            // NoteOff at the last sample before clip_end (not at clip_end itself,
                            // since clip_end is exclusive). When clip_end == buf_end, the offset
                            // would be frames-1 (last sample of the buffer).
                            let offset = clip_end.saturating_sub(1).saturating_sub(buf_start) as u32;
                            if offset < frames as u32 {
                                let pckn = Pckn::new(0u8, 0u8, note.pitch, 0u8);
                                buf.push(&NoteOffEvent::new(offset, pckn, 0.0));
                            }
                        }
                    }

                    for cc in &clip.midi_cc_events {
                        let cc_timeline =
                            (clip_start as i64 + cc.time_frames as i64 - clip_off as i64).max(0)
                                as u64;
                        if cc_timeline >= buf_start && cc_timeline < buf_end && cc_timeline <= clip_end {
                            let offset = (cc_timeline - buf_start) as u32;
                            if offset < frames as u32 {
                                let val_7bit = (cc.value * 127.0).round().clamp(0.0, 127.0) as u8;
                                buf.push(&MidiEvent::new(offset, 0, [0xB0, cc.cc_number, val_7bit]));
                            }
                        }
                    }
                }
                buf.sort();
                let n_events = buf.len();
                if n_events > 0 {
                    tracing::trace!(n_events, "dispatching MIDI events to instrument");
                }
                let events = buf.as_input();
                a.process_with_events(mix_l, mix_r, sample_rate, &events);

                let mut has_output = false;
                for (l, r) in mix_l[..frames].iter_mut().zip(mix_r[..frames].iter_mut()) {
                    if !l.is_finite() {
                        *l = 0.0;
                    } else {
                        *l = l.clamp(-10.0, 10.0);
                    }
                    if !r.is_finite() {
                        *r = 0.0;
                    } else {
                        *r = r.clamp(-10.0, 10.0);
                    }
                    if !has_output {
                        has_output = l.abs() > 1e-10 || r.abs() > 1e-10;
                    }
                    *l *= track_vol;
                    *r *= track_vol;
                }
                if n_events > 0 && !has_output {
                    tracing::warn!(
                        "instrument process_with_events produced zero output after {} events",
                        n_events
                    );
                }
            });
        } else {
            adapter.lock().ok();
        }
    }

    Some(ii)
}
