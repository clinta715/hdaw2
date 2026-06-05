use crate::app::undo::UndoCommand;
use crate::app::HdawApp;
use crate::project::clip::ClipKind;
use crate::project::clip_handle::ClipHandle;

impl HdawApp {
    pub fn add_midi_note(&mut self, track_index: usize, clip_id: uuid::Uuid, note: crate::project::midi_note::MidiNote) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                clip.notes.push(note.clone());
                clip.notes.sort_by_key(|n| n.start_frame);
                clip.thumb_dirty = true;
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    ch.midi_notes.push(note.clone());
                    ch.midi_notes.sort_by_key(|n| n.start_frame);
                }
            }
        }
        self.undo_service.push(UndoCommand::AddMidiNote {
            track_index,
            clip_id,
            note,
        });
    }

    pub fn remove_midi_note(&mut self, track_index: usize, clip_id: uuid::Uuid, note_idx: usize) {
        let note = self.project.tracks.get(track_index).and_then(|track| {
            track.clips.iter().find_map(|c| match c {
                ClipKind::Midi(m) if m.id == clip_id => m.notes.get(note_idx).cloned(),
                _ => None,
            })
        });
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                if note_idx < clip.notes.len() {
                    clip.notes.remove(note_idx);
                    clip.thumb_dirty = true;
                }
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    if note_idx < ch.midi_notes.len() {
                        ch.midi_notes.remove(note_idx);
                    }
                }
            }
        }
        if let Some(note) = note {
            self.undo_service.push(UndoCommand::RemoveMidiNote {
                track_index,
                clip_id,
                note,
                note_index: note_idx,
            });
        }
    }

    pub fn update_midi_note(&mut self, track_index: usize, clip_id: uuid::Uuid, note_idx: usize, new_note: crate::project::midi_note::MidiNote) {
        let old_note = self.project.tracks.get(track_index).and_then(|track| {
            track.clips.iter().find_map(|c| match c {
                ClipKind::Midi(m) if m.id == clip_id => m.notes.get(note_idx).cloned(),
                _ => None,
            })
        });

        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                if note_idx < clip.notes.len() {
                    clip.notes[note_idx] = new_note.clone();
                    clip.thumb_dirty = true;
                }
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    if note_idx < ch.midi_notes.len() {
                        ch.midi_notes[note_idx] = new_note.clone();
                    }
                }
            }
        }

        if let Some(old) = old_note {
             self.undo_service.push(UndoCommand::UpdateMidiNote {
                track_index,
                clip_id,
                note_index: note_idx,
                old_note: old,
                new_note,
            });
        }
    }

    pub fn add_midi_cc_event(&mut self, track_index: usize, clip_id: uuid::Uuid, event: crate::project::cc_event::CCEvent) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                clip.cc_events.push(event.clone());
                clip.cc_events.sort_by_key(|e| e.time_frames);
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    ch.midi_cc_events.push(event.clone());
                    ch.midi_cc_events.sort_by_key(|e| e.time_frames);
                }
            }
        }
        self.undo_service.push(UndoCommand::AddCcEvent { track_index, clip_id, event });
    }

    pub fn remove_midi_cc_event(&mut self, track_index: usize, clip_id: uuid::Uuid, event: &crate::project::cc_event::CCEvent, event_index: usize) {
        let event_clone = event.clone();
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                clip.cc_events.retain(|e| e.cc_number != event.cc_number || e.time_frames != event.time_frames);
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    ch.midi_cc_events.retain(|e| e.cc_number != event.cc_number || e.time_frames != event.time_frames);
                }
            }
        }
        self.undo_service.push(UndoCommand::RemoveCcEvent { track_index, clip_id, event: event_clone, event_index });
    }

    pub fn update_midi_cc_event(&mut self, track_index: usize, clip_id: uuid::Uuid, old_event: &crate::project::cc_event::CCEvent, new_event: crate::project::cc_event::CCEvent) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                if let Some(ctx) = clip.cc_events.iter_mut().find(|e| e.cc_number == old_event.cc_number && e.time_frames == old_event.time_frames) {
                    *ctx = new_event.clone();
                }
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    if let Some(ctx) = ch.midi_cc_events.iter_mut().find(|e| e.cc_number == old_event.cc_number && e.time_frames == old_event.time_frames) {
                        *ctx = new_event.clone();
                    }
                }
            }
        }
    }

    pub fn add_midi_clip(&mut self, track_index: usize, position_frames: u64, length_frames: u64) {
        let clip_id = uuid::Uuid::new_v4();
        let midi_clip = crate::project::midi_clip::MidiClip {
            id: clip_id,
            name: "MIDI Clip".into(),
            position_frames,
            length_frames,
            notes: Vec::new(),
            color: [0x8a, 0x2b, 0xe2],
            cc_events: Vec::new(),
            thumb_dirty: true,
        };
        let clip = ClipKind::Midi(midi_clip);

        if let Some(track) = self.project.tracks.get_mut(track_index) {
            track.add_clip(clip.clone());
        }

        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                let sr = self.engine.transport.sample_rate();
                let clip_handle = ClipHandle::new_midi(clip_id, Vec::new(), length_frames, sr);
                clip_handle.set_position(position_frames);
                handle.add_clip(clip_handle);
            }
        }

        if let Some(tui) = self.track_ui.get_mut(track_index) {
            tui.color = [0x2a, 0x1a, 0x3a];
        }

        self.undo_service.push(UndoCommand::AddMidiClip {
            track_index,
            clip,
        });
    }
}
