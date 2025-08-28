use crate::db::VoteCount;
use paho_mqtt::{ConnectOptionsBuilder, CreateOptionsBuilder, Message, QoS, QOS_2};
use serde::Deserialize;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, process, thread};
use std::ops::Add;
use tokio::sync::Mutex;

static MQTT_HOST: LazyLock<String> =
    LazyLock::new(|| env::var("MQTT_HOST").expect("MQTT_HOST not present"));

pub static EMPTY_SONG: LazyLock<SongInfo> = LazyLock::new(|| SongInfo {
    title: String::from(""),
    artist: String::from(""),
    cover_img: String::from(""),
    song_id: String::from(""),
    paused_on: SystemTime::UNIX_EPOCH,
});

#[derive(Clone)]
pub struct SongInfo {
    pub title: String,
    pub artist: String,
    pub cover_img: String,
    pub song_id: String,
    pub paused_on: SystemTime,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuitarSongInfo {
    name: String,
    spotify_id: String,
    image_url: Option<String>,
    artists: Vec<String>,
}


fn try_reconnect(cli: &paho_mqtt::Client) -> bool
{
    println!("Connection lost. Waiting to retry connection");
    for _ in 0..12 {
        thread::sleep(Duration::from_millis(5000));
        if cli.reconnect().is_ok() {
            println!("Successfully reconnected");
            return true;
        }
    }
    println!("Unable to reconnect after several attempts.");
    false
}

pub fn init_client() -> paho_mqtt::Client {
    let create_opts = CreateOptionsBuilder::new()
        .server_uri(MQTT_HOST.to_string())
        .client_id("ZODOM".to_string())
        .finalize();

    let cli = paho_mqtt::Client::new(create_opts).unwrap_or_else(|err| {
        println!("Error creating the MQTT client: {:?}", err);
        process::exit(1);
    });

    let conn_opts = ConnectOptionsBuilder::new()
        .keep_alive_interval(Duration::from_secs(20))
        .clean_session(false)
        .finalize();

    if let Err(e) = cli.connect(conn_opts) {
        println!("Unable to connect to MQTT server:\n\t{:?}", e);
        process::exit(1);
    }

    cli
}

async fn listen(client: Arc<paho_mqtt::Client>, last_song: Arc<Mutex<SongInfo>>, current_song: Arc<Mutex<SongInfo>>) {
    let queue = client.start_consuming();
    subscribe_topics(&client);
    println!("Started Listening");
    for msg in queue.iter() {
        if let Some(msg) = msg {
            println!("Received Msg: {}", msg);
            if msg.topic() == "music/current_song_info" {
                let payload: GuitarSongInfo = serde_json::from_str(&*msg.payload_str()).unwrap();

                update_songs(last_song.clone(), current_song.clone(), SongInfo {
                    title: payload.name,
                    artist: payload.artists.join(", "),
                    cover_img: payload.image_url.unwrap_or(String::from("/static/assets/cover-placeholder.svg")),
                    song_id: payload.spotify_id,
                    paused_on: SystemTime::UNIX_EPOCH,
                }).await
            } else if msg.topic() == "music/events/paused" || msg.topic() == "music/events/stopped" {
                pause_song(last_song.clone(), current_song.clone()).await;
            }
        } else if !client.is_connected() {
            if try_reconnect(&client) {
                subscribe_topics(&client)
            }
        }
    }
}

fn subscribe_topics(client: &paho_mqtt::Client) {
    let topics = &["music/events/paused", "music/events/stopped", "music/current_song_info"];
    client.subscribe_many_same_qos(topics, QOS_2).unwrap();
}

pub fn start_listening(client: Arc<paho_mqtt::Client>, last_song: Arc<Mutex<SongInfo>>, current_song: Arc<Mutex<SongInfo>>) {
    println!("Spawning Listen Thread");
    tokio::spawn(async { listen(client, last_song, current_song).await });
}

pub fn publish_vote_update(client: &paho_mqtt::Client, vote_count: VoteCount) {
    let msg = Message::new(
        "music/votes",
        serde_json::to_string(&vote_count).unwrap(),
        QoS::ExactlyOnce,
    );
    client.publish(msg).unwrap();
}

async fn update_songs(last_song: Arc<Mutex<SongInfo>>, current_song: Arc<Mutex<SongInfo>>, new_song: SongInfo) {
    let mut last_guard = last_song.lock().await;
    let mut current_guard = current_song.lock().await;

    // Within Timeframe
    if current_guard.paused_on.add(Duration::from_secs(60 * 15)) > SystemTime::now() {
        dbg!("Back From Pause");
        if new_song.song_id == last_guard.song_id {
            std::mem::swap(&mut *last_guard, &mut *current_guard);
        } else {
            *current_guard = new_song;
        }
        // Music Is Already Playing
    } else if current_guard.paused_on == UNIX_EPOCH {
        if new_song.song_id == last_guard.song_id {
            dbg!("No Swaps");
            std::mem::swap(&mut *last_guard, &mut *current_guard);
            *current_guard = new_song;
        }

        // Out Of Timeframe
    } else {
        dbg!("Reset Every Song");
        *last_guard = EMPTY_SONG.clone();
        *current_guard = new_song;
    }
}

async fn pause_song(last_song: Arc<Mutex<SongInfo>>, current_song: Arc<Mutex<SongInfo>>) {
    dbg!("Pausing song");
    let mut last_guard = last_song.lock().await;
    let mut current_guard = current_song.lock().await;

    std::mem::swap(&mut *last_guard, &mut *current_guard);

    current_guard.paused_on = SystemTime::now();
}