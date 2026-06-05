use crate::app::HdawApp;
use crate::project::clip::{AudioClip, ClipKind};
use crate::project::clip_handle::ClipHandle;

impl HdawApp {
    pub fn restore_pool_clip_to_track(&mut self, pool_clip_id: uuid::Uuid, track_index: usize) {
        self.restore_pool_clip_to_track_at(pool_clip_id, track_index, 0);
    }

    pub fn restore_pool_clip_to_track_at(&mut self, pool_clip_id: uuid::Uuid, track_index: usize, position_frames: u64) {
        let pool_idx = self.project.audio_pool.iter()
            .position(|p| p.id == pool_clip_id);
        let pool_clip = match pool_idx {
            Some(idx) => self.project.audio_pool.remove(idx),
            None => return,
        };

        let (buffer, clip_id) = match &pool_clip.clip {
            ClipKind::Audio(a) => match a.buffer.clone() {
                Some(buf) => (buf, a.id),
                None => return,
            },
            ClipKind::Midi(_) => return,
        };

        let clip_handle = ClipHandle::new(
            clip_id,
            (*buffer.samples()).to_vec(),
            buffer.channels(),
            buffer.sample_rate(),
        );
        clip_handle.set_position(position_frames);

        let mut audio_clip = AudioClip::new(pool_clip.name.clone(), buffer);
        audio_clip.position_frames = position_frames;

        if let Some(track) = self.project.tracks.get_mut(track_index) {
            track.add_clip(ClipKind::Audio(audio_clip));
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                handle.add_clip(clip_handle);
            }
        }
    }
}
