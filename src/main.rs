mod auth;
mod db;
mod songs;
mod error;
mod music_manager;

use std::process::exit;
use crate::auth::ZauthUser;
use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::post;
use axum::{debug_handler, routing::get, Form, Json, Router};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::Mutex;
use tower_http::services::ServeDir;
use tower_sessions::{cookie::SameSite, Session, SessionManagerLayer};
use tower_sessions_file_store::FileSessionStorage;
use crate::error::AppError;
use crate::music_manager::{MusicManager, MusicState};

#[derive(Clone)]
struct AppState {
    music_manager: MusicManager,
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
    music_state: &'a MusicState,
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
        music_manager: MusicManager{
            music_state: Arc::new(Mutex::from(MusicState {
                current_song: None,
                last_song: None,
                last_last_song: None,
                paused_at: Some(0), // paused_at Should not be None when current_song is None
            }))},
        db: db::create_client().await,
        mqtt_client: Arc::new(songs::init_client()),
    };
    let _ = db::create_tables(&app_state.db).await;

    songs::start_listening(Arc::clone(&app_state.mqtt_client), app_state.music_manager.clone().await);

    let static_files = ServeDir::new("./static/");
    let session_store = FileSessionStorage::new();
    let session_layer = SessionManagerLayer::new(session_store).with_same_site(SameSite::Lax);

    let app = Router::new()
        .route("/", get(index))
        .route("/login", get(auth::login))
        .route("/oauth/callback", get(auth::callback))
        .route("/logout", get(auth::logout))
        .route("/submit_vote", post(submit_vote))
        .route("/vote_count/{song_id}", get(get_vote_count))
        .route("/current_song", get(get_current_song_or_paused))
        .layer(session_layer)
        .nest_service("/static", static_files)
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.expect("Couldn't Start Listener");
    println!("Server on {}", listener.local_addr().map(|x| x.to_string()).unwrap_or("[unknown]".to_string()));
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await.expect("Couldn't Start Server");
}

async fn index(session: Session, State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    Ok(match session.get::<ZauthUser>("user").await? {
        None => login()?.into_response(),
        Some(user) => {
            let music_state = state.music_manager.read_state().await;
            let last_song = &music_state.last_song;
            let current_song = &music_state.current_song;
            let current_song_vote = db::get_vote(&state.db, user.id, &*current_song.as_ref().unwrap().song_id).await?; // TODO Handle Unwrap
            let last_song_vote = db::get_vote(&state.db, user.id, &*last_song.as_ref().unwrap().song_id).await?; // TODO Handle Unwrap
            let desk_template = VotePageTemplate {
                user: &user,
                music_state: &music_state,
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
            let _ = db::add_vote(&state.db, user.id, payload.likes, &*payload.song_id).await;
            songs::publish_vote_update(&state.mqtt_client, db::get_vote_count(&state.db, &*payload.song_id).await?);
            Redirect::to("/")
        }
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VoteCountRequest {
    song_id: String,
}

async fn get_vote_count(State(state): State<AppState>, Path(song_id): Path<String>) -> Result<impl IntoResponse, AppError> {
    Ok(Json(db::get_vote_count(&state.db, &*song_id).await?))
}

async fn get_current_song_or_paused(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.music_manager.read_state().await.current_song.as_ref().unwrap().song_id.clone()) // TODO Handle Unwrap
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("Shutdown signal received, shutting down gracefully...");
    exit(0);
}