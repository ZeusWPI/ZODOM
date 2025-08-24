use std::{env, process, thread};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use paho_mqtt::{ConnectOptionsBuilder, CreateOptionsBuilder, Message, QoS, QOS_2};
use crate::db::VoteCount;

static MQTT_HOST: LazyLock<String> =
    LazyLock::new(|| env::var("MQTT_HOST").expect("MQTT_HOST not present"));

pub struct SongInfo {
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) cover_image: String,
    pub(crate) song_id: String,
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

fn listen(client: Arc<paho_mqtt::Client>) {
    subscribe_topics(&client);
    let queue = client.start_consuming();
    println!("Started Listening");
    for msg in queue.iter() {
        if let Some(msg) = msg {
            println!("{}", msg);
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

pub fn start_listening(client: Arc<paho_mqtt::Client>) {
    thread::spawn(|| { listen(client); });
}

pub fn publish_vote_update(client: &paho_mqtt::Client, vote_count: VoteCount) {
    let msg = Message::new(
        "music/votes",
        serde_json::to_string(&vote_count).unwrap(),
        QoS::ExactlyOnce,
    );
    client.publish(msg).unwrap();
}

