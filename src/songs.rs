use std::{env, process, thread};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use axum::Json;
use paho_mqtt::{ConnectOptionsBuilder, CreateOptionsBuilder, Message, QoS, QOS_2};
use serde::Deserialize;
use tokio::sync::Mutex;
use crate::db::VoteCount;

static MQTT_HOST: LazyLock<String> =
    LazyLock::new(|| env::var("MQTT_HOST").expect("MQTT_HOST not present"));

pub struct SongInfo {
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) cover_image: String,
    pub(crate) song_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuitarSongInfo {
    name: String,
    album: String,
    length_in_ms: u32,
    ends_at: u32,
    spotify_id: String,
    image_url: String,
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
    subscribe_topics(&client);
    let queue = client.start_consuming();
    println!("Started Listening");
    for msg in queue.iter() {
        if let Some(msg) = msg {
            println!("Received Msg: {}", msg);
            if msg.topic() == "music/play" {
                let payload: GuitarSongInfo = serde_json::from_str(&*msg.payload_str()).unwrap();

                update_songs(last_song.clone(), current_song.clone(), SongInfo {
                    title: payload.name,
                    artist: payload.artists.join(", "),
                    cover_image: payload.image_url,
                    song_id: payload.spotify_id,
                }).await
            }
        } else if !client.is_connected() {
            if try_reconnect(&client) {
                subscribe_topics(&client)
            }
        }
    }
}

fn subscribe_topics(client: &paho_mqtt::Client) {
    let topics = &["music/pause", "music/play"];
    client.subscribe_many(topics, &[QOS_2, QOS_2]).unwrap();
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

    std::mem::swap(&mut *last_guard, &mut *current_guard);

    *current_guard = new_song;
}