use rumqttc::{AsyncClient, MqttOptions, QoS, Event, Packet};
use std::time::Duration;
use sqlx::SqlitePool;
use tokio::sync::broadcast;

use crate::models::NovaLeitura;
use crate::db::{inserir_leitura, buscar_leitura};

const MQTT_BROKER: &str = "localhost";
const MQTT_PORT: u16 = 1883;
const MQTT_TOPIC: &str = "esp32/sensores";

pub async fn start_mqtt_client(pool: SqlitePool, tx: broadcast::Sender<String>) {
    let mut mqttoptions = MqttOptions::new("estacao_meteo_server", MQTT_BROKER, MQTT_PORT);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    client.subscribe(MQTT_TOPIC, QoS::AtMostOnce).await.unwrap();
    println!("📡 MQTT subscrito em '{MQTT_TOPIC}' via {MQTT_BROKER}");

    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(publish))) => {
                match serde_json::from_slice::<NovaLeitura>(&publish.payload) {
                    Ok(nova) => {
                        println!("📥 MQTT: {:?}", nova);
                        match inserir_leitura(&pool, &nova).await {
                            Ok(id) => {
                                // Busca a leitura completa (com id e timestamp gerado pelo banco)
                                // e faz broadcast para todos os WebSocket clients conectados
                                if let Ok(leitura) = buscar_leitura(&pool, id).await {
                                    if let Ok(json) = serde_json::to_string(&leitura) {
                                        // Ignora erro se não há receivers conectados
                                        let _ = tx.send(json);
                                    }
                                }
                            }
                            Err(e) => eprintln!("❌ Erro ao salvar leitura MQTT: {e}"),
                        }
                    }
                    Err(e) => eprintln!("⚠️  JSON MQTT inválido: {e}"),
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("🔌 Erro MQTT: {e}. Reconectando em 5s...");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
