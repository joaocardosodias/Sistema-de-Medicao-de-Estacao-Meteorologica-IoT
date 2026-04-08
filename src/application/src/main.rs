mod api;
mod db;
mod models;
mod mqtt;

use actix_files::Files;
use actix_web::{web, App, HttpServer};
use tokio::sync::broadcast;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let _ = dotenvy::dotenv();

    let db_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:dados.db?mode=rwc".to_string());

    let pool = db::init_db(&db_url)
        .await
        .expect("Falha ao inicializar o banco de dados");
    println!("Banco de dados inicializado.");

    let (tx, _) = broadcast::channel::<String>(256);
    let tx_data = web::Data::new(tx.clone());

    let mqtt_pool = pool.clone();
    let mqtt_tx = tx.clone();
    tokio::spawn(async move {
        mqtt::start_mqtt_client(mqtt_pool, mqtt_tx).await;
    });

    println!("Servidor rodando em http://0.0.0.0:8080");

    let app_data = web::Data::new(pool);

    HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone())
            .app_data(tx_data.clone())
            .configure(api::config)
            .service(Files::new("/", "./static").index_file("index.html"))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
