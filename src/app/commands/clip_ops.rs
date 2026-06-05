use crate::app::undo::UndoCommand;
use crate::app::HdawApp;
use crate::project::clip::ClipKind;
use std::sync::atomic::Ordering;

impl HdawApp {
    pub fn update_clip_position(&mut self, track_index: usize, clip_id: uuid::Uuid, new_position: u64) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            for clip in track.clips.iter_mut() {
                match clip {
                    ClipKind::Audio(a) if a.id == clip_id => a.position_frames = new_position,
                    ClipKind::Midi(m) if m.id == clip_id => m.position_frames = new_position,
                    _ => continue,
                }
                break;
            }
        }
        if let Ok(tracks) = self.engine.tracks.lock() {
            if let Some(track) = tracks.get(track_index) {
                if let Some(ch) = track.clips.iter().find(|c| c.clip_id == clip_id) {
                    ch.set_position(new_position);
                }
            }
        }
    }

    pub fn move_clip_to_track(&mut self, clip_id: uuid::Uuid, from: usize, to: usize, position: u64) {
        let removed = if let Some(track) = self.project.tracks.get_mut(from) {
            let idx = track.clips.iter().position(|c| match c {
                ClipKind::Audio(a) => a.id == clip_id,
                ClipKind::Midi(m) => m.id == clip_id,
            });
            idx.map(|i| track.clips.remove(i))
        } else { None };
        if let Some(mut c) = removed {
            match &mut c {
                ClipKind::Audio(a) => a.position_frames = position,
                ClipKind::Midi(m) => m.position_frames = position,
            }
            if let Some(track) = self.project.tracks.get_mut(to) {
                track.add_clip(c);
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            let clip_handle = if from < tracks.len() {
                let pos = tracks[from].clips.iter().position(|c| c.clip_id == clip_id);
                pos.map(|p| tracks[from].clips.remove(p))
            } else { None };
            if let Some(ch) = clip_handle {
                ch.set_position(position);
                if to < tracks.len() {
                    tracks[to].clips.push(ch);
                }
            }
        }
    }

    pub fn update_clip_trim(&mut self, track_index: usize, clip_id: uuid::Uuid, position: Option<u64>, offset: Option<u64>, length: Option<u64>) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            for clip in track.clips.iter_mut() {
                match clip {
                    ClipKind::Audio(a) if a.id == clip_id => {
                        if let Some(p) = position { a.position_frames = p; }
                        if let Some(o) = offset { a.offset_frames = o; }
                        if let Some(l) = length { a.length_frames = l; }
                    }
                    ClipKind::Midi(m) if m.id == clip_id => {
                        if let Some(p) = position { m.position_frames = p; }
                        if let Some(l) = length { m.length_frames = l; }
                    }
                    _ => continue,
                }
                break;
            }
        }
        if let Ok(tracks) = self.engine.tracks.lock() {
            if let Some(track) = tracks.get(track_index) {
                if let Some(ch) = track.clips.iter().find(|c| c.clip_id == clip_id) {
                    if let Some(p) = position { ch.set_position(p); }
                    if let Some(o) = offset { ch.set_offset(o); }
                    if let Some(l) = length { ch.set_length(l); }
                }
            }
        }
    }

    pub fn remove_selected_clip(&mut self) {
        let clip_id = match self.timeline_state.selected_clip_id {
            Some(id) => id,
            None => return,
        };
        for ti in 0..self.project.tracks.len() {
            if let Some(track) = self.project.tracks.get(ti) {
                if let Some(ci) = track.clips.iter().position(|c| match c {
                    ClipKind::Audio(a) => a.id == clip_id,
                    ClipKind::Midi(m) => m.id == clip_id,
                }) {
                    let clip = track.clips[ci].clone();
                    if let Some(track) = self.project.tracks.get_mut(ti) {
                        track.clips.remove(ci);
                    }
                    if let Ok(mut tracks) = self.engine.tracks.lock() {
                        if let Some(handle) = tracks.get_mut(ti) {
                            if let Some(eng_idx) = handle.find_clip_by_id(clip_id) {
                                handle.clips.remove(eng_idx);
                            }
                        }
                    }
                    self.waveform_cache.remove(&clip_id);
                    self.midi_thumb_cache.remove(&clip_id);
                    let undo_clip = match &clip {
                        ClipKind::Audio(a) => {
                            let mut stripped = a.clone();
                            stripped.buffer = None;
                            ClipKind::Audio(stripped)
                        }
                        other => other.clone(),
                    };
                    self.undo_service.push(UndoCommand::DeleteClip {
                        track_index: ti,
                        clip_index: ci,
                        clip: undo_clip,
                    });
                    break;
                }
            }
        }
        self.timeline_state.selected_clip_id = None;
    }

    pub fn split_selected_clip(&mut self) {
        let clip_id = match self.timeline_state.selected_clip_id {
            Some(id) => id,
            None => return,
        };
        let pos = self.engine.transport.position_frames();
        for ti in 0..self.project.tracks.len() {
            if self.project.tracks[ti].clips.iter().any(|c| match c {
                ClipKind::Audio(a) => a.id == clip_id,
                ClipKind::Midi(m) => m.id == clip_id,
            }) {
                self.split_clip_at(ti, clip_id, pos);
                return;
            }
        }
    }

    pub fn update_clip_fade(&mut self, track_index: usize, clip_id: uuid::Uuid, fade_in: Option<u64>, fade_out: Option<u64>) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            for clip in track.clips.iter_mut() {
                match clip {
                    ClipKind::Audio(a) if a.id == clip_id => {
                        if let Some(fi) = fade_in { a.fade_in_frames = fi; }
                        if let Some(fo) = fade_out { a.fade_out_frames = fo; }
                    }
                    _ => continue,
                }
                break;
            }
        }
        if let Ok(tracks) = self.engine.tracks.lock() {
            if let Some(track) = tracks.get(track_index) {
                if let Some(ch) = track.clips.iter().find(|c| c.clip_id == clip_id) {
                    if let Some(fi) = fade_in { ch.fade_in_frames.store(fi, Ordering::Release); }
                    if let Some(fo) = fade_out { ch.fade_out_frames.store(fo, Ordering::Release); }
                }
            }
        }
    }

    pub fn duplicate_clip(&mut self, track_index: usize, clip_id: uuid::Uuid) {
        let Some(track) = self.project.tracks.get(track_index) else { return };
        let Some((clip, new_id)) = track.clips.iter().find_map(|c| match c {
            ClipKind::Audio(a) if a.id == clip_id => {
                let mut new = a.clone();
                let new_id = uuid::Uuid::new_v4();
                new.id = new_id;
                new.position_frames = a.position_frames + a.length_frames;
                Some((ClipKind::Audio(new), new_id))
            }
            ClipKind::Midi(m) if m.id == clip_id => {
                let mut new = m.clone();
                let new_id = uuid::Uuid::new_v4();
                new.id = new_id;
                new.position_frames = m.position_frames + m.length_frames;
                Some((ClipKind::Midi(new), new_id))
            }
            _ => None,
        }) else { return };
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            track.add_clip(clip.clone());
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            let sr = self.engine.transport.sample_rate();
            let ch = match &clip {
                ClipKind::Audio(a) => {
                    let buffer = a.buffer.as_ref().map(|b| (**b.samples()).clone()).unwrap_or_default();
                    let ch = crate::project::clip_handle::ClipHandle::new(new_id, buffer, a.buffer.as_ref().map(|b| b.channels()).unwrap_or(0), a.buffer.as_ref().map(|b| b.sample_rate()).unwrap_or(sr));
                    ch.set_position(a.position_frames);
                    ch
                }
                ClipKind::Midi(m) => {
                    let ch = crate::project::clip_handle::ClipHandle::new_midi(new_id, m.notes.clone(), m.length_frames, sr);
                    ch.set_position(m.position_frames);
                    ch
                }
            };
            if let Some(handle) = tracks.get_mut(track_index) {
                handle.add_clip(ch);
            }
        }
        self.undo_service.push(UndoCommand::DuplicateClip { track_index, clip_id, new_clip_id: new_id });
    }

    pub fn split_clip_at(&mut self, track_index: usize, clip_id: uuid::Uuid, split_frame: u64) {
        let (pos, off, len, audio_buf, midi_notes) = {
            let track = match self.project.tracks.get(track_index) { Some(t) => t, None => return };
            let found = track.clips.iter().find_map(|c| match c {
                ClipKind::Audio(a) if a.id == clip_id => {
                    Some((a.position_frames, a.offset_frames, a.length_frames, a.buffer.clone(), None))
                }
                ClipKind::Midi(m) if m.id == clip_id => {
                    let notes = m.notes.clone();
                    Some((m.position_frames, 0, m.length_frames, None, Some(notes)))
                }
                _ => None,
            });
            match found { Some(v) => v, None => return }
        };
        let split_local = split_frame.saturating_sub(pos);
        if split_local <= off || split_local >= off + len { return; }
        let left_len = split_local - off;
        let right_len = off + len - split_local;
        let right_id = uuid::Uuid::new_v4();
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(ClipKind::Audio(a)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Audio(a) if a.id == clip_id)) {
                a.length_frames = left_len;
            }
            if let Some(ClipKind::Midi(m)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                m.length_frames = left_len;
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    ch.set_length(left_len);
                }
            }
        }
        let audio_for_engine = audio_buf.clone();
        let right_clip = match (audio_buf, midi_notes) {
            (Some(buf), _) => {
                let mut a = crate::project::clip::AudioClip::new("split".into(), buf);
                a.id = right_id;
                a.position_frames = split_frame;
                a.offset_frames = split_local;
                a.length_frames = right_len;
                ClipKind::Audio(a)
            }
            (_, Some(orig_notes)) => {
                let local_split = split_frame - pos;
                let right_notes: Vec<_> = orig_notes.iter()
                    .filter(|n| n.start_frame >= local_split)
                    .map(|n| {
                        let mut nn = n.clone();
                        nn.start_frame = nn.start_frame.saturating_sub(local_split);
                        nn
                    })
                    .collect();
                let m = crate::project::midi_clip::MidiClip {
                    id: right_id,
                    name: "split".into(),
                    position_frames: split_frame,
                    length_frames: right_len,
                    notes: right_notes,
                    color: [0x8a, 0x2b, 0xe2],
                    cc_events: Vec::new(),
                    thumb_dirty: true,
                };
                ClipKind::Midi(m)
            }
            _ => return,
        };
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            track.add_clip(right_clip);
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                let sr = self.engine.transport.sample_rate();
                let right_ch = match audio_for_engine {
                    Some(ref buf) => {
                        let ch = crate::project::clip_handle::ClipHandle::new(right_id, (**buf.samples()).clone(), buf.channels(), buf.sample_rate());
                        ch.set_offset(split_local);
                        ch.set_position(split_frame);
                        ch.set_length(right_len);
                        ch
                    }
                    None => {
                        let ch = crate::project::clip_handle::ClipHandle::new_midi(right_id, vec![], right_len, sr);
                        ch.set_position(split_frame);
                        ch
                    }
                };
                handle.add_clip(right_ch);
            }
        }
        self.undo_service.push(UndoCommand::SplitClip {
            track_index,
            clip_id,
            new_clip_id: right_id,
            old_length: len,
            left_length: left_len,
            right_length: right_len,
        });
    }

    pub fn glue_clips(&mut self, track_index: usize, clip_a_id: uuid::Uuid, clip_b_id: uuid::Uuid) {
        let clip_data = if let Some(track) = self.project.tracks.get(track_index) {
            let a = track.clips.iter().find(|c| match c {
                ClipKind::Audio(a) => a.id == clip_a_id,
                ClipKind::Midi(m) => m.id == clip_a_id,
            }).cloned();
            let b = track.clips.iter().find(|c| match c {
                ClipKind::Audio(a) => a.id == clip_b_id,
                ClipKind::Midi(m) => m.id == clip_b_id,
            }).cloned();
            (a, b)
        } else { (None, None) };
        let (clip_a, clip_b) = match clip_data {
            (Some(a), Some(b)) => (a, b),
            _ => return,
        };

        let merged_id = uuid::Uuid::new_v4();
        let merged = match (&clip_a, &clip_b) {
            (ClipKind::Audio(a), ClipKind::Audio(b)) => {
                let sr = a.buffer.as_ref().map(|buf| buf.sample_rate()).unwrap_or(44100);
                let channels = a.buffer.as_ref().map(|buf| buf.channels()).unwrap_or(2);
                let pos = a.position_frames.min(b.position_frames);
                let _end_a = a.position_frames + a.length_frames;
                let end_b = b.position_frames + b.length_frames;
                let len = end_b - pos;
                let total_samples = len as usize * channels as usize;
                let mut merged_samples = vec![0.0f32; total_samples];

                let mut fill_clip = |clip: &crate::project::clip::AudioClip, _start_offset: u64| {
                    if let Some(ref buf) = clip.buffer {
                        let samples = buf.samples();
                        let ch = buf.channels() as usize;
                        let clip_start_sample = clip.offset_frames as usize * ch;
                        let clip_len_samples = clip.length_frames as usize * ch;
                        let copy_len = clip_len_samples.min(samples.len().saturating_sub(clip_start_sample));
                        let merged_start_sample = (clip.position_frames - pos) as usize * ch;
                        for j in 0..copy_len {
                            let dst = merged_start_sample + j;
                            if dst < merged_samples.len() {
                                merged_samples[dst] = samples[clip_start_sample + j];
                            }
                        }
                    }
                };
                fill_clip(a, a.position_frames);
                fill_clip(b, b.position_frames);

                let buffer = crate::audio::buffer::AudioBuffer::from_interleaved(merged_samples, channels, sr);
                let mut merged = crate::project::clip::AudioClip::new("glued".into(), buffer);
                merged.id = merged_id;
                merged.position_frames = pos;
                merged.offset_frames = 0;
                merged.length_frames = len;
                ClipKind::Audio(merged)
            }
            (ClipKind::Midi(a), ClipKind::Midi(b)) => {
                let pos = a.position_frames.min(b.position_frames);
                let end_b = b.position_frames + b.length_frames;
                let len = end_b - pos;
                let mut merged_notes: Vec<crate::project::midi_note::MidiNote> = a.notes.iter().map(|n| {
                    let mut nn = n.clone();
                    nn.start_frame -= a.position_frames - pos;
                    nn
                }).collect();
                let offset_b = b.position_frames - pos;
                merged_notes.extend(b.notes.iter().map(|n| {
                    let mut nn = n.clone();
                    nn.start_frame += offset_b;
                    nn
                }));
                let m = crate::project::midi_clip::MidiClip {
                    id: merged_id,
                    name: "glued".into(),
                    position_frames: pos,
                    length_frames: len,
                    notes: merged_notes,
                    color: [0x6a, 0x4a, 0x9a],
                    cc_events: Vec::new(),
                    thumb_dirty: true,
                };
                ClipKind::Midi(m)
            }
            _ => return,
        };

        if let Some(track) = self.project.tracks.get_mut(track_index) {
            track.clips.retain(|c| match c {
                ClipKind::Audio(a) => a.id != clip_a_id && a.id != clip_b_id,
                ClipKind::Midi(m) => m.id != clip_a_id && m.id != clip_b_id,
            });
            track.add_clip(merged.clone());
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                handle.clips.retain(|c| c.clip_id != clip_a_id && c.clip_id != clip_b_id);
                let sr = self.engine.transport.sample_rate();
                let ch = match &merged {
                    ClipKind::Audio(a) => {
                        let buf_data = a.buffer.as_ref().map(|b| (**b.samples()).clone()).unwrap_or_default();
                        let ch = crate::project::clip_handle::ClipHandle::new(
                            merged_id, buf_data,
                            a.buffer.as_ref().map(|b| b.channels()).unwrap_or(2),
                            a.buffer.as_ref().map(|b| b.sample_rate()).unwrap_or(sr),
                        );
                        ch.set_position(a.position_frames);
                        ch
                    }
                    ClipKind::Midi(m) => {
                        let ch = crate::project::clip_handle::ClipHandle::new_midi(merged_id, m.notes.clone(), m.length_frames, sr);
                        ch.set_position(m.position_frames);
                        ch
                    }
                };
                handle.add_clip(ch);
            }
        }
        self.undo_service.push(UndoCommand::GlueClips {
            track_index,
            clip_a: clip_a.clone(),
            clip_b: clip_b.clone(),
            merged_clip: merged,
        });
    }
}
