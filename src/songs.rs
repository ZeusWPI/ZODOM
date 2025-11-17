use crate::db::VoteCount;
use paho_mqtt::{ConnectOptionsBuilder, CreateOptionsBuilder, Message, QoS, QOS_2};
use serde::Deserialize;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use std::{env, thread};
use crate::music_manager::{MusicManager, SongInfo};

static MQTT_HOST: LazyLock<String> =
    LazyLock::new(|| env::var("MQTT_HOST").expect("MQTT_HOST not present"));
static MQTT_CLIENT_ID: LazyLock<String> =
    LazyLock::new(|| env::var("MQTT_CLIENT_ID").expect("MQTT_CLIENT_ID not present"));



#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuitarSongInfo {
    name: String,
    spotify_id: String,
    image_url: Option<String>,
    artists: Vec<String>,
    started_at_ms: u64,
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
        .client_id(MQTT_CLIENT_ID.to_string())
        .finalize();

    let cli = paho_mqtt::Client::new(create_opts).expect("Error creating the MQTT client");

    let conn_opts = ConnectOptionsBuilder::new()
        .keep_alive_interval(Duration::from_secs(20))
        .clean_session(false)
        .finalize();

    cli.connect(conn_opts).expect("Unable to connect to MQTT server");

    cli
}

async fn listen(client: Arc<paho_mqtt::Client>, mut music_manager: MusicManager) {
    let queue = client.start_consuming();
    subscribe_topics(&client);
    println!("Started Listening");
    for msg in queue.iter() {
        if let Some(msg) = msg {
            println!("Received Msg: {}", msg);
            match msg.topic() {
                "music/current_song_info" => {
                    if let Ok(payload) = serde_json::from_str::<GuitarSongInfo>(&*msg.payload_str()) {
                        music_manager.new_song(SongInfo {
                            title: payload.name,
                            artist: payload.artists.join(", "),
                            cover_img: payload.image_url.unwrap_or(String::from("/static/assets/cover-placeholder.svg")),
                            song_id: payload.spotify_id,
                            started_at: payload.started_at_ms / 1000,
                        }).await
                    }
                }
                "music/events/paused" | "music/events/stopped" => { music_manager.pause().await }
                _ => {}
            }
        } else if !client.is_connected() && try_reconnect(&client) {
            subscribe_topics(&client)
        }
    }
}


fn subscribe_topics(client: &paho_mqtt::Client) {
    let topics = &["music/events/paused", "music/events/stopped", "music/current_song_info"];
    client.subscribe_many_same_qos(topics, QOS_2).expect("Failed To Subscribe to MQTT Topics");
}

pub fn start_listening(client: Arc<paho_mqtt::Client>, music_manager: MusicManager) {
    println!("Spawning Listen Thread");
    tokio::spawn(async { listen(client, music_manager).await });
}

pub fn publish_vote_update(client: &paho_mqtt::Client, vote_count: VoteCount) {
    let msg = Message::new(
        "music/votes",
        serde_json::to_string(&vote_count).unwrap(),
        QoS::ExactlyOnce,
    );
    client.publish(msg).unwrap_or_else(|_| { println!("Couldn't Publish Vote Update") });
}
