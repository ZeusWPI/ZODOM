mod auth;
mod db;
mod songs;
mod error;

use std::ops::Add;
use crate::auth::ZauthUser;
use crate::songs::SongInfo;
use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::post;
use axum::{debug_handler, routing::get, Error, Form, Json, Router};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use tokio::sync::Mutex;
use tower_http::services::ServeDir;
use tower_sessions::{cookie::SameSite, Session, SessionManagerLayer};
use tower_sessions_file_store::FileSessionStorage;
use crate::error::AppError;

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
#[template(path = "vote_page.askama")]
struct VotePageTemplate<'a> {
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
            song_id: String::from(""),
            title: String::from(""),
            artist: String::from(""),
            cover_img: String::from(""),
            paused_on: UNIX_EPOCH.add(Duration::from_secs(1)),
        })),
        last_song: Arc::new(Mutex::new(songs::EMPTY_SONG.clone())),
        db: db::create_client().await,
        mqtt_client: Arc::new(songs::init_client()),
    };
    db::create_tables(&app_state.db).await;

    songs::start_listening(Arc::clone(&app_state.mqtt_client), Arc::clone(&app_state.last_song), Arc::clone(&app_state.current_song));

    let static_files = ServeDir::new("./static/");
    let session_store = FileSessionStorage::new();
    let session_layer = SessionManagerLayer::new(session_store).with_same_site(SameSite::Lax).with_secure(false); //TODO Remove secure(false)

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

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.expect("Couldn't Start Listener");
    println!("Server on {}", listener.local_addr().map(|x| x.to_string()).unwrap_or("[unknown]".to_string()));
    axum::serve(listener, app).await.expect("Couldn't Start Server");
}

async fn index(session: Session, State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    Ok(match session.get::<ZauthUser>("user").await? {
        None => login()?.into_response(),
        Some(user) => {
            let last_song = state.last_song.lock().await;
            let current_song = state.current_song.lock().await;
            let current_song_vote = db::get_vote(&state.db, user.id, &*current_song.song_id).await;
            let last_song_vote = db::get_vote(&state.db, user.id, &*last_song.song_id).await;
            let desk_template = VotePageTemplate {
                user: &user,
                current_song: &*current_song,
                last_song: &*last_song,
                current_song_vote,
                last_song_vote,
            };
            Html(desk_template.render()?).into_response()
        }
    })
}

fn login() -> Result<impl IntoResponse, AppError> {
    let login_template = LoginTemplate {};
    Ok(Html(login_template.render()?))
}

#[debug_handler]
async fn submit_vote(
    session: Session,
    State(state): State<AppState>,
    Form(payload): Form<VoteSubmission>,
) -> Result<Redirect, AppError> {
    Ok(match session.get::<ZauthUser>("user").await? {
        None => Redirect::to("/"),
        Some(user) => {
            db::add_vote(&state.db, user.id, payload.likes, &*payload.song_id).await;
            songs::publish_vote_update(&state.mqtt_client, db::get_vote_count(&state.db, &*payload.song_id).await);
            Redirect::to("/")
        }
    })
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