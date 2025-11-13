use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct SongInfo {
    pub title: String,
    pub artist: String,
    pub cover_img: String,
    pub song_id: String,
    pub started_at: u64,
}
#[derive(Clone)]
pub struct MusicState {
    pub current_song: Option<SongInfo>,
    pub last_song: Option<SongInfo>,
    pub last_last_song: Option<SongInfo>,
    pub paused_at: Option<u64>
}

#[derive(Clone)]
pub struct MusicManager {
    pub(crate) music_state: Arc<Mutex<MusicState>>
}
impl MusicManager {
    pub(crate) async fn clone(&self) -> MusicManager{
        MusicManager {
            music_state: self.music_state.clone()
        }
    }
    pub(crate) async fn read_state(&self) -> MusicState {
        let state = self.music_state.lock().await;
        MusicState {
            current_song: state.current_song.clone(),
            last_song: state.last_song.clone(),
            last_last_song: state.last_last_song.clone(),
            paused_at: state.paused_at.clone(),
        }
    }

    pub async fn pause(&self) {
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let mut music_state = self.music_state.lock().await;
        
        music_state.shift(None);
        music_state.paused_at = Some(current_time);
    }

    pub async fn new_song(&mut self, new_song: SongInfo) {

        let mut music_state = self.music_state.lock().await;

        if let Some(paused_at) = music_state.paused_at {
            let secs_passed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - paused_at;
            if secs_passed >= 15 * 60 {
                music_state.clear();
                music_state.shift(Some(new_song));
                music_state.paused_at = None;

            } else if let Some(last_song) = &music_state.last_song
                && last_song.song_id == new_song.song_id {
                if last_song.started_at == new_song.started_at { return; } // Duplicate MQTT Message

                // Song came back from pause
                music_state.unshift();
                music_state.paused_at = None;

            } else { // New Song After Pause
                music_state.current_song = Some(new_song);
                music_state.paused_at = None;
            }

        } else { // Not Paused
            if let Some(current_song) = &music_state.current_song
                && current_song.song_id == new_song.song_id && current_song.started_at == new_song.started_at {
                    // Duplicate MQTT Message
                    return
            } else {
                music_state.shift(Some(new_song))
            }
        }
    }
}

impl MusicState {
    pub fn shift(&mut self, new_song: Option<SongInfo>) {
        self.last_last_song = self.last_song.clone();
        self.last_song = self.current_song.clone();
        self.current_song = new_song;
        self.paused_at = None;
    }

    pub fn unshift(&mut self) {
        self.current_song = self.last_song.clone();
        self.last_song = self.last_last_song.clone();
        self.last_last_song = None;
    }

    pub fn clear(&mut self) {
        self.current_song = None;
        self.last_song = None;
        self.last_last_song = None;
        self.paused_at = Some(0);
    }
}