use std::env;
use std::sync::LazyLock;
use serde::Serialize;
use sqlx::{Error, Executor, Row, SqlitePool};
use sqlx::sqlite::SqliteConnectOptions;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteCount {
    pub song_id: String,
    pub votes_for: u32,
    pub votes_against: u32,
}

static DB_LOCATION: LazyLock<String> =
    LazyLock::new(|| env::var("DB_LOCATION").expect("DB_LOCATION not present"));


pub async fn create_client() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(DB_LOCATION.as_str())
        .create_if_missing(true);

    SqlitePool::connect_with(options).await.expect("Couldn't Connect To Database")
}

pub async fn create_tables(db: &SqlitePool) -> Result<(), Error> {
    db.execute(
        "CREATE TABLE IF NOT EXISTS votes (
        userId INTEGER,
        songId VARCHAR(255),
        likes BOOL,
        voted_on DATETIME DEFAULT current_timestamp,
        PRIMARY KEY(userId, songId)
        )"
    ).await?;
    Ok(())
}


pub async fn add_vote(db: &SqlitePool, user_id: u32, likes: bool, song_id: &str) -> Result<(), Error> {
    sqlx::query(
        "
        INSERT INTO votes(userId, songId, likes)
        VALUES(?, ?, ?)
        ON CONFLICT(userId, songId) DO
        UPDATE SET likes = ?, voted_on = current_timestamp;
",
    )
        .bind(user_id)
        .bind(song_id.to_string())
        .bind(likes)
        .bind(likes)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn get_vote(db: &SqlitePool, user_id: u32, song_id: &str) -> Result<Option<bool>, Error> {
    let likes = sqlx::query(
        "
    SELECT likes FROM votes WHERE userId = ? AND songId = ?;
",
    )
        .bind(user_id)
        .bind(song_id.to_string())
        .fetch_optional(db)
        .await?;

    Ok(if let Some(likes) = likes {
        likes.get(0)
    } else {
        None
    })
}

pub async fn get_vote_count(db: &SqlitePool, song_id: &str) -> Result<VoteCount, Error> {
    let query = sqlx::query(
        "
    SELECT
        count(CASE WHEN likes THEN 1 END) as votes_for,
        count(CASE WHEN NOT likes THEN 1 END) as votes_against
    FROM votes WHERE songId = ?
"
    )
        .bind(song_id)
        .fetch_one(db)
        .await?;

    Ok(VoteCount {
        song_id: song_id.to_string(),
        votes_for: query.get(0),
        votes_against: query.get(1),
    })
}
