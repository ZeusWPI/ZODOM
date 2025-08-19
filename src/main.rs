use askama::Template;
use axum::response::{Html, IntoResponse};
use axum::{Router, routing::get};

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate<'a> {
    name: &'a str,
}

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = Router::new().route("/", get(hello));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn hello() -> impl IntoResponse {
    let hello = HelloTemplate { name: "Hannes" }; // instantiate your struct
    Html(hello.render().unwrap())
}
