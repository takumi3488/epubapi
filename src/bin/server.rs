use epubapi::{
    db::db::{connect_db, insert_admin_user},
    routes::routes::init_app,
};

/// Main function
///
/// Start axum server
#[tokio::main]
async fn main() {
    let db = connect_db().await;
    insert_admin_user(&db).await;
    let router = init_app(&db);
    let listner = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listner, router).await.unwrap();
}
