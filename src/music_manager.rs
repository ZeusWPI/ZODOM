use std::sync::Arc;
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
        let mut music_state = self.music_state.lock().await;
        music_state.shift(None);

    }

    pub async fn new_song(&mut self, new_song: SongInfo) {

        let mut music_state = self.music_state.lock().await;

        //TODO
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
}