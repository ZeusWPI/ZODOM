mod auth;
mod db;
mod songs;

use crate::auth::ZauthUser;
use crate::songs::SongInfo;
use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse};
use axum::{Router, routing::get};
use std::sync::{Arc, Mutex, MutexGuard};
use tower_http::services::ServeDir;
use tower_sessions::{Session, SessionManagerLayer, cookie::SameSite};
use tower_sessions_file_store::FileSessionStorage;

#[derive(Clone)]
struct AppState {
    current_song: Arc<Mutex<SongInfo>>,
    last_song: Arc<Mutex<SongInfo>>,
}

#[derive(Template)]
#[template(path = "desktop.html")]
struct DesktopTemplate<'a> {
    current_song_title: &'a str,
    current_song_artist: &'a str,
    last_song_title: &'a str,
    last_song_artist: &'a str,
    username: &'a str,
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {}

#[tokio::main]
async fn main() {
    let app_state = AppState {
        current_song: Arc::new(Mutex::new(SongInfo {
            title: String::new(),
            artist: String::new(),
            cover_image: String::new(),
        })),
        last_song: Arc::new(Mutex::new(SongInfo {
            title: String::new(),
            artist: String::new(),
            cover_image: String::new(),
        })),
    };

    let static_files = ServeDir::new("./static/");
    let session_store = FileSessionStorage::new();
    let session_layer = SessionManagerLayer::new(session_store).with_same_site(SameSite::Lax);

    let app = Router::new()
        .route("/", get(index))
        .route("/login", get(auth::login))
        .route("/oauth/callback", get(auth::callback))
        .route("/logout", get(auth::logout))
        .layer(session_layer)
        .nest_service("/static", static_files)
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("server on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn index(session: Session, State(state): State<AppState>) -> impl IntoResponse {
    let user = session.get::<ZauthUser>("user").await.unwrap();
    if user.is_none() {
        return login().into_response();
    }
    let last_song = state.last_song.lock().expect("mutex was poisoned");

    if let Some(user) = user {
        let desk_template = DesktopTemplate {
            username: &*user.username,
            current_song_title: &*String::from("Current Song Title"),
            current_song_artist: &*String::from("Current Song Artist"),
            last_song_title: &*last_song.title,
            last_song_artist: &*last_song.artist,
        }; // instantiate your struct
        return Html(desk_template.render().unwrap()).into_response();
    } else {
        panic!("Should Never Happen")
    }
}

fn login() -> impl IntoResponse {
    let login_template = LoginTemplate {};
    Html(login_template.render().unwrap())
}
