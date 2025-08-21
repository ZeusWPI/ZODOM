use sqlx::{Connection, Executor, SqliteConnection};

pub async fn add_vote(username: &str, likes: bool) {
    let mut conn = SqliteConnection::connect("sqlite://test.db").await.unwrap();

    conn.execute(sqlx::query(
        "INSERT INTO votes(user, likes) VALUES ('nathano', true)",
    ))
    .await
    .unwrap();
}
