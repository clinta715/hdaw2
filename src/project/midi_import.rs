use crate::project::midi_note::MidiNote;

#[derive(Debug, Clone)]
pub struct ImportedMidiTrack {
    pub name: String,
    pub notes: Vec<MidiNote>,
    pub duration_frames: u64,
}

/// Parse a Standard MIDI File and convert its tracks to HDAW MidiNotes.
/// Uses project BPM + sample rate for tick-to-frame conversion.
pub fn parse_midi_file(path: &str, bpm: f64, sample_rate: u32) -> Result<Vec<ImportedMidiTrack>, String> {
    use midly::Smf;
    let data = std::fs::read(path).map_err(|e| format!("cannot read MIDI file: {e}"))?;
    let smf = Smf::parse(&data).map_err(|e| format!("invalid MIDI file: {e}"))?;

    let ppqn = match smf.header.timing {
        midly::Timing::Metrical(ticks) => ticks.as_int() as f64,
        midly::Timing::Timecode(_, _) => return Err("timecode-based MIDI files not supported".to_string()),
    };

    let frames_per_tick = if ppqn > 0.0 {
        (sample_rate as f64 * 60.0) / (bpm * ppqn)
    } else {
        return Err("MIDI file has zero ticks per quarter note".to_string());
    };

    let mut tracks: Vec<ImportedMidiTrack> = Vec::new();

    for track in &smf.tracks {
        let (notes, duration_ticks) = extract_notes(track, frames_per_tick, sample_rate)?;
        if notes.is_empty() {
            continue;
        }
        let track_name = find_track_name(track);
        let duration_frames = (duration_ticks as f64 * frames_per_tick).ceil() as u64;
        tracks.push(ImportedMidiTrack {
            name: track_name.unwrap_or_else(|| format!("Track {}", tracks.len() + 1)),
            notes,
            duration_frames,
        });
    }

    if tracks.is_empty() {
        return Err("no MIDI notes found in file".to_string());
    }

    Ok(tracks)
}

fn find_track_name(track: &midly::Track) -> Option<String> {
    let first = track.first()?;
    if let midly::TrackEventKind::Meta(meta) = &first.kind {
        if let midly::MetaMessage::TrackName(name) = meta {
            return Some(
                std::str::from_utf8(name).unwrap_or("").to_string(),
            );
        }
    }
    None
}

fn extract_notes(
    track: &midly::Track,
    frames_per_tick: f64,
    _sample_rate: u32,
) -> Result<(Vec<MidiNote>, u64), String> {
    // Active notes: map from pitch -> (velocity, start_tick)
    use std::collections::HashMap;
    let mut active_notes: HashMap<u8, (u8, u64)> = HashMap::new();
    let mut notes: Vec<MidiNote> = Vec::new();
    let mut abs_tick: u64 = 0;

    for ev in track {
        let delta: u64 = ev.delta.as_int() as u64;
        abs_tick = abs_tick.checked_add(delta).ok_or("overflow in MIDI tick count")?;

        match &ev.kind {
            midly::TrackEventKind::Midi { channel: _, message } => {
                match message {
                    midly::MidiMessage::NoteOn { key, vel } => {
                        let pitch = key.as_int();
                        let velocity = vel.as_int();
                        if velocity == 0 {
                            if let Some((_start_vel, start_tick)) = active_notes.remove(&pitch) {
                                let duration = abs_tick.saturating_sub(start_tick);
                                if duration > 0 {
                                    notes.push(MidiNote {
                                        pitch,
                                        velocity: _start_vel,
                                        start_frame: (start_tick as f64 * frames_per_tick).round() as u64,
                                        duration: (duration as f64 * frames_per_tick).round() as u64,
                                    });
                                }
                            }
                        } else {
                            if let Some((old_vel, start_tick)) = active_notes.remove(&pitch) {
                                let duration = abs_tick.saturating_sub(start_tick);
                                if duration > 0 {
                                    notes.push(MidiNote {
                                        pitch,
                                        velocity: old_vel,
                                        start_frame: (start_tick as f64 * frames_per_tick).round() as u64,
                                        duration: (duration as f64 * frames_per_tick).round() as u64,
                                    });
                                }
                            }
                            active_notes.insert(pitch, (velocity, abs_tick));
                        }
                    }
                    midly::MidiMessage::NoteOff { key, vel } => {
                        let pitch = key.as_int();
                        if let Some((start_vel, start_tick)) = active_notes.remove(&pitch) {
                            let duration = abs_tick.saturating_sub(start_tick);
                            if duration > 0 {
                                notes.push(MidiNote {
                                    pitch,
                                    velocity: start_vel,
                                    start_frame: (start_tick as f64 * frames_per_tick).round() as u64,
                                    duration: (duration as f64 * frames_per_tick).round() as u64,
                                });
                            }
                        } else if vel.as_int() > 0 {
                            notes.push(MidiNote {
                                pitch,
                                velocity: vel.as_int(),
                                start_frame: (abs_tick as f64 * frames_per_tick).round() as u64,
                                duration: 1,
                            });
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // Close any remaining active notes at the end of track.
    // Use the tick at the last event for a minimum duration of 1 frame.
    let end_tick = abs_tick;
    for (pitch, (velocity, start_tick)) in active_notes.drain() {
        let duration = end_tick.saturating_sub(start_tick).max(1);
        notes.push(MidiNote {
            pitch,
            velocity,
            start_frame: (start_tick as f64 * frames_per_tick).round() as u64,
            duration: (duration as f64 * frames_per_tick).ceil() as u64,
        });
    }

    notes.sort_by_key(|n| n.start_frame);

    Ok((notes, end_tick))
}
