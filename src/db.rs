use sqlx::{Connection, Executor, Row, SqliteConnection, SqlitePool};

pub async fn add_vote(db: SqlitePool, user_id: u32, likes: bool, song_id: &str) {
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
    .execute(&db)
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
