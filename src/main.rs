mod auth;
mod db;
mod songs;

use crate::auth::ZauthUser;
use crate::songs::SongInfo;
use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::post;
use axum::{routing::get, Form, Json, Router};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tokio::sync::Mutex;
use tower_http::services::ServeDir;
use tower_sessions::{cookie::SameSite, Session, SessionManagerLayer};
use tower_sessions_file_store::FileSessionStorage;

#[derive(Clone)]
struct AppState {
    current_song: Arc<Mutex<SongInfo>>,
    last_song: Arc<Mutex<SongInfo>>,
    db: SqlitePool,
    mqtt_client: Arc<paho_mqtt::Client>,
}

#[derive(Deserialize)]
struct VoteSubmission {
    song_id: String,
    likes: bool,
}

#[derive(Template)]
#[template(path = "desktop.askama")]
struct DesktopTemplate<'a> {
    current_song: &'a SongInfo,
    last_song: &'a SongInfo,
    user: &'a ZauthUser,
    current_song_vote: Option<bool>,
    last_song_vote: Option<bool>,
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {}

#[tokio::main]
async fn main() {
    let app_state = AppState {
        current_song: Arc::new(Mutex::new(SongInfo {
            song_id: String::from("3LQY0O87BlaOKMp56ST4hC"),
            title: String::from("Une vie à t'aimer"),
            artist: String::from("Lorien Testard, Alice Duport-Percier, Victor Borba"),
            cover_img: String::from("/static/assets/placeholders/song_cover_2.jpg"),
            paused_on: UNIX_EPOCH,
        })),
        last_song: Arc::new(Mutex::new(SongInfo {
            song_id: String::from("4QEXM9na0mWIIt5Hwbsges"),
            title: String::from("No Time to Explain"),
            artist: String::from("Good Kid"),
            cover_img: String::from("/static/assets/placeholders/song_cover.jpg"),
            paused_on: UNIX_EPOCH,
        })),
        db: SqlitePool::connect("sqlite:test.db").await.unwrap(),
        mqtt_client: Arc::new(songs::init_client()),
    };

    songs::start_listening(Arc::clone(&app_state.mqtt_client), Arc::clone(&app_state.last_song), Arc::clone(&app_state.current_song));

    let static_files = ServeDir::new("./static/");
    let session_store = FileSessionStorage::new();
    let session_layer = SessionManagerLayer::new(session_store).with_same_site(SameSite::Lax);

    let app = Router::new()
        .route("/", get(index))
        .route("/login", get(auth::login))
        .route("/oauth/callback", get(auth::callback))
        .route("/logout", get(auth::logout))
        .route("/submit_vote", post(submit_vote))
        .route("/vote_count", get(get_vote_count))
        .route("/current_song", get(get_current_song_or_paused))
        .layer(session_layer)
        .nest_service("/static", static_files)
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("server on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn index(session: Session, State(state): State<AppState>) -> impl IntoResponse {
    // TODO: Remove Debug Statement
    let debug_vote_count = db::get_vote_count(&state.db, "3LQY0O87BlaOKMp56ST4hC").await;
    println!("Une vie à t'aimer vote count: {} For, {} Against", debug_vote_count.votes_for, debug_vote_count.votes_against);
    let user = session.get::<ZauthUser>("user").await.unwrap();
    if user.is_none() {
        return login().into_response();
    }
    let last_song = state.last_song.lock().await;
    let current_song = state.current_song.lock().await;

    if let Some(user) = user {
        let current_song_vote = db::get_vote(&state.db, user.id, &*current_song.song_id).await;
        let last_song_vote = db::get_vote(&state.db, user.id, &*last_song.song_id).await;

        let desk_template = DesktopTemplate {
            user: &user,
            current_song: &*current_song,
            last_song: &*last_song,
            current_song_vote,
            last_song_vote,
        };
        Html(desk_template.render().unwrap()).into_response()
    } else {
        panic!("Should Never Happen")
    }
}

fn login() -> impl IntoResponse {
    let login_template = LoginTemplate {};
    Html(login_template.render().unwrap())
}

async fn submit_vote(
    session: Session,
    State(state): State<AppState>,
    Form(payload): Form<VoteSubmission>,
) -> Redirect {
    match session.get::<ZauthUser>("user").await.unwrap() {
        None => Redirect::to("/"),
        Some(user) => {
            db::add_vote(&state.db, user.id, payload.likes, &*payload.song_id).await;
            songs::publish_vote_update(&state.mqtt_client, db::get_vote_count(&state.db, &*payload.song_id).await);
            Redirect::to("/")
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VoteCountRequest {
    song_id: String,
}

async fn get_vote_count(State(state): State<AppState>, Json(payload): Json<VoteCountRequest>) -> impl IntoResponse {
    Json(db::get_vote_count(&state.db, &payload.song_id).await)
}

async fn get_current_song_or_paused(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.current_song.lock().await.song_id.clone())
}