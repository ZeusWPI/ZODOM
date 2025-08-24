use serde::Serialize;
use sqlx::{Row, SqlitePool};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteCount {
    pub song_id: String,
    pub votes_for: u32,
    pub votes_against: u32,
}

pub async fn add_vote(db: &SqlitePool, user_id: u32, likes: bool, song_id: &str) {
    println!(
        "User <{}> voted {} song <{}>",
        user_id,
        if likes { "For" } else { "Against" },
        song_id
    );

    sqlx::query(
        "
        INSERT INTO votes(userId, songId, likes)
        VALUES(?, ?, ?)
        ON CONFLICT(userId, songId) DO
        UPDATE SET likes = ?;
",
    )
        .bind(user_id)
        .bind(song_id.to_string())
        .bind(likes)
        .bind(likes)
        .execute(db)
        .await
        .unwrap();
}

pub async fn get_vote(db: &SqlitePool, user_id: u32, song_id: &str) -> Option<bool> {
    let likes = sqlx::query(
        "
    SELECT likes FROM votes WHERE userId = ? AND songId = ?;
",
    )
        .bind(user_id)
        .bind(song_id.to_string())
        .fetch_optional(db)
        .await
        .unwrap();

    if let Some(likes) = likes {
        likes.get(0)
    } else {
        None
    }
}

pub async fn get_vote_count(db: &SqlitePool, song_id: &str) -> VoteCount {
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
        .await
        .unwrap();

    VoteCount {
        song_id: song_id.to_string(),
        votes_for: query.get(0),
        votes_against: query.get(1),
    }
}
