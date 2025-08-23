use std::{env, process};
use std::sync::LazyLock;
use std::time::Duration;
use axum::Json;
use paho_mqtt::{ConnectOptionsBuilder, CreateOptionsBuilder, Message, QoS};
use crate::db::VoteCount;

static MQTT_HOST: LazyLock<String> =
    LazyLock::new(|| env::var("MQTT_HOST").expect("MQTT_HOST not present"));

pub struct SongInfo {
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) cover_image: String,
    pub(crate) song_id: String,
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

pub(crate) fn publish_vote_update(client: paho_mqtt::Client, vote_count: VoteCount) {
    let msg = Message::new(
        "music/votes",
        serde_json::to_string(&vote_count).unwrap(),
        QoS::ExactlyOnce,
    );
    client.publish(msg).unwrap();
}
