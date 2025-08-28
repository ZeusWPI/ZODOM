use std::{env, sync::LazyLock};

use axum::{
    extract::Query,
    response::{IntoResponse, Redirect},
};
use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::error::AppError;

static ZAUTH_URL: LazyLock<String> =
    LazyLock::new(|| env::var("ZAUTH_URL").expect("ZAUTH_URL not present"));
static CALLBACK_URL: LazyLock<String> =
    LazyLock::new(|| env::var("ZAUTH_CALLBACK_PATH").expect("ZAUTH_CALLBACK_PATH not present"));
static ZAUTH_CLIENT_ID: LazyLock<String> =
    LazyLock::new(|| env::var("ZAUTH_CLIENT_ID").expect("ZAUTH_CLIENT_ID not present"));
static ZAUTH_CLIENT_SECRET: LazyLock<String> =
    LazyLock::new(|| env::var("ZAUTH_CLIENT_SECRET").expect("ZAUTH_CLIENT_SECRET not present"));


pub async fn login(session: Session) -> Result<Redirect, AppError> {
    let state = Alphanumeric.sample_string(&mut rand::rng(), 16);
    // insert state so we can check it in the callback
    session.insert("state", state.clone()).await?;
    // redirect to zauth to authenticate
    let zauth_url = ZAUTH_URL.to_string();
    let callback_url = CALLBACK_URL.to_string();
    let zauth_client_id = ZAUTH_CLIENT_ID.to_string();
    Ok(Redirect::to(&format!(
        "{zauth_url}/oauth/authorize?client_id={zauth_client_id}&response_type=code&state={state}&redirect_uri={callback_url}"
    )))
}

pub async fn logout(session: Session) -> impl IntoResponse {
    session.clear().await;
    Redirect::to("/")
}

pub async fn callback(
    Query(params): Query<Callback>,
    session: Session,
) -> Result<Redirect, AppError> {
    let zauth_state = match session.get::<String>("state").await? {
        None => return Ok(Redirect::to("/login")),
        Some(v) => v,
    };

    // check if saved state matches returned state
    if zauth_state != params.state {
        return Err(AppError::Zauth("States don't match".into()));
    }

    let callback_url = CALLBACK_URL.to_string();
    let client = reqwest::Client::new();
    let form = [
        ("grant_type", "authorization_code"),
        ("code", &params.code),
        ("redirect_uri", &callback_url),
    ];

    let zauth_url = ZAUTH_URL.to_string();
    // get token from zauth with code
    let token = client
        .post(format!("{zauth_url}/oauth/token"))
        .basic_auth(
            ZAUTH_CLIENT_ID.to_string(),
            Some(ZAUTH_CLIENT_SECRET.to_string()),
        )
        .form(&form)
        .send()
        .await?
        .json::<ZauthToken>()
        .await?;

    // get user info from zauth
    let zauth_user = client
        .get(format!("{zauth_url}/current_user"))
        .header("Authorization", "Bearer ".to_owned() + &token.access_token)
        .send()
        .await?
        .error_for_status()?
        .json::<ZauthUser>()
        .await?;

    session.clear().await;
    session.insert("user", zauth_user).await?;
    Ok(Redirect::to("/"))
}


#[derive(Deserialize, Debug)]
pub struct Callback {
    state: String,
    code: String,
}

#[derive(Deserialize, Debug)]
pub struct ZauthToken {
    access_token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ZauthUser {
    pub id: u32,
    pub username: String,
}