mod auth;
mod db;
mod songs;

use crate::auth::ZauthUser;
use crate::songs::SongInfo;
use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::post;
use axum::{Form, Json, Router, debug_handler, routing::get};
use serde::Deserialize;
use sqlx::{SqliteConnection, SqlitePool};
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};
use tower_http::services::ServeDir;
use tower_sessions::{Session, SessionManagerLayer, cookie::SameSite};
use tower_sessions_file_store::FileSessionStorage;

#[derive(Clone)]
struct AppState {
    current_song: Arc<Mutex<SongInfo>>,
    last_song: Arc<Mutex<SongInfo>>,
    db: SqlitePool,
}

#[derive(Deserialize)]
struct VoteSubmission {
    song_id: String,
    likes: bool,
}

#[derive(Template)]
#[template(path = "desktop.askama")]
struct DesktopTemplate<'a> {
    current_song_id: &'a str,
    current_song_title: &'a str,
    current_song_artist: &'a str,
    last_song_id: &'a str,
    last_song_title: &'a str,
    last_song_artist: &'a str,
    username: &'a str,
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
            cover_image: String::from("/static/assets/placeholders/song_cover_2.jpg"),
        })),
        last_song: Arc::new(Mutex::new(SongInfo {
            song_id: String::from("4QEXM9na0mWIIt5Hwbsges"),
            title: String::from("No Time to Explain"),
            artist: String::from("Good Kid"),
            cover_image: String::from("/static/assets/placeholders/song_cover.jpg"),
        })),
        db: SqlitePool::connect("sqlite:test.db").await.unwrap(),
    };

    let static_files = ServeDir::new("./static/");
    let session_store = FileSessionStorage::new();
    let session_layer = SessionManagerLayer::new(session_store).with_same_site(SameSite::Lax);

    let app = Router::new()
        .route("/", get(index))
        .route("/login", get(auth::login))
        .route("/oauth/callback", get(auth::callback))
        .route("/logout", get(auth::logout))
        .route("/vote", post(submit_vote))
        .layer(session_layer)
        .nest_service("/static", static_files)
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("server on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[debug_handler]
async fn index(session: Session, State(state): State<AppState>) -> impl IntoResponse {
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
            username: &*user.username,
            current_song_title: &*current_song.title,
            current_song_artist: &*current_song.artist,
            current_song_id: &*current_song.song_id,
            last_song_id: &*last_song.song_id,
            last_song_title: &*last_song.title,
            last_song_artist: &*last_song.artist,
            current_song_vote,
            last_song_vote,
        };
        return Html(desk_template.render().unwrap()).into_response();
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
            db::add_vote(state.db, user.id, payload.likes, &*payload.song_id).await;
            Redirect::to("/")
        }
    }
}
