//! Simulador MQTT para a Estação Meteorológica IoT
//!
//! Publica leituras aleatórias realistas no tópico MQTT do servidor,
//! simulando o comportamento do firmware ESP32.
//!
//! Uso:
//!   cargo run --bin simulador
//!
//! Variáveis de ambiente opcionais:
//!   MQTT_BROKER    (padrão: localhost)
//!   MQTT_PORT      (padrão: 1883)
//!   MQTT_TOPIC     (padrão: esp32/sensores)
//!   INTERVAL_SECS  (padrão: 5)

use rand::Rng;
use rumqttc::{Client, MqttOptions, QoS};
use std::thread;
use std::time::Duration;

const LOCALIZACOES: &[&str] = &["Lab", "Telhado", "Jardim", "Corredor", "Sala de Servidores"];

fn main() {
    // ── Configuração via variáveis de ambiente ─────────────
    let broker   = std::env::var("MQTT_BROKER").unwrap_or_else(|_| "localhost".into());
    let port     = std::env::var("MQTT_PORT")
        .ok().and_then(|p| p.parse().ok()).unwrap_or(1883u16);
    let topic    = std::env::var("MQTT_TOPIC")
        .unwrap_or_else(|_| "esp32/sensores".into());
    let interval = std::env::var("INTERVAL_SECS")
        .ok().and_then(|i| i.parse().ok()).unwrap_or(5u64);

    // ── Conexão MQTT (API síncrona) ────────────────────────
    let mut opts = MqttOptions::new("simulador-estacao-rust", &broker, port);
    opts.set_keep_alive(Duration::from_secs(10));

    let (client, mut connection) = Client::new(opts, 64);

    // A API síncrona do rumqttc exige que os eventos sejam consumidos
    // em uma thread separada para não travar o publish
    thread::spawn(move || {
        for event in connection.iter() {
            match event {
                Ok(ev) => {
                    // Mostra apenas conexão estabelecida
                    if matches!(ev, rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(_))) {
                        println!("✅ Conectado ao broker MQTT em {broker}:{port}");
                    }
                }
                Err(e) => eprintln!("⚠️  MQTT: {e}"),
            }
        }
    });

    // Aguarda a conexão ser processada
    thread::sleep(Duration::from_millis(500));

    // ── Banner ─────────────────────────────────────────────
    println!("🌡️  Simulador Estação Meteorológica — Rust Edition");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("   Tópico   : {topic}");
    println!("   Intervalo: {interval}s | Ctrl+C para encerrar");
    println!();

    let mut rng   = rand::thread_rng();
    let mut count = 0u64;

    loop {
        // ── Geração de valores realistas ───────────────────
        let temperatura = rng.gen_range(18.0_f64..38.0);
        let umidade     = rng.gen_range(30.0_f64..95.0);
        let localizacao = LOCALIZACOES[rng.gen_range(0..LOCALIZACOES.len())];
        let tem_pressao = rng.gen_bool(0.6); // 60% das leituras têm pressão

        let payload = if tem_pressao {
            let pressao = rng.gen_range(1005.0_f64..1025.0);
            format!(
                r#"{{"temperatura":{temperatura:.1},"umidade":{umidade:.1},"pressao":{pressao:.1},"localizacao":"{localizacao}"}}"#
            )
        } else {
            format!(
                r#"{{"temperatura":{temperatura:.1},"umidade":{umidade:.1},"localizacao":"{localizacao}"}}"#
            )
        };

        // ── Publicação ─────────────────────────────────────
        count += 1;
        match client.publish(&topic, QoS::AtLeastOnce, false, payload.as_bytes()) {
            Ok(_)  => println!("[{count:>4}] ✓  {payload}"),
            Err(e) => eprintln!("[{count:>4}] ✗  Erro ao publicar: {e}"),
        }

        thread::sleep(Duration::from_secs(interval));
    }
}
