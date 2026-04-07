//! Firmware ESP32 — Estação Meteorológica IoT
//!
//! Versão robusta: utiliza uma thread separada com stack dedicada (32KB)
//! para evitar o erro de Stack Overflow da main task do ESP-IDF.

use anyhow::{bail, Context, Result};
use esp_idf_hal::{
    delay::Ets,
    gpio::{PinDriver, Pull},
    i2c::{I2cConfig, I2cDriver},
    peripherals::Peripherals,
    units::KiloHertz,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    mqtt::client::{EspMqttClient, MqttClientConfiguration, QoS},
    wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use esp_idf_sys as _;
use log::{error, info, warn};

use bme280::i2c::BME280;
use dht_sensor::dht11::blocking::read as dht11_read;

use std::{thread, time::Duration};

// ─── Configuração ────────────────────────────────────────────────────────────

macro_rules! cfg_str {
    ($key:literal, $default:literal) => {
        match option_env!($key) {
            Some(v) => v,
            None    => $default,
        }
    };
}

const WIFI_SSID:   &str = cfg_str!("CONFIG_WIFI_SSID",       "Joao");
const WIFI_PASS:   &str = cfg_str!("CONFIG_WIFI_PASS",       "vaitomarnocu");
const MQTT_BROKER: &str = cfg_str!("CONFIG_MQTT_BROKER_URL", "mqtt://10.211.123.89:1883");
const MQTT_TOPIC:  &str = "esp32/sensores";
const MQTT_ID:     &str = "esp32-estacao-meteo";
const LOCALIZACAO: &str = "Lab";
const INTERVALO:   Duration = Duration::from_secs(5);

// ─── Entrada Principal ───────────────────────────────────────────────────────

fn main() -> Result<()> {
    // Inicialização básica do sistema
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    // Inicializa o NVS (necessário para o WiFi carregar dados de calibração PHY)
    unsafe {
        let _ = esp_idf_sys::nvs_flash_init();
    }

    info!("🚀 Iniciando Estação Meteorológica...");

    // Criamos uma thread dedicada com 32KB de stack.
    // Isso é muito mais seguro que depender da stack padrão da 'main task'.
    let builder = thread::Builder::new()
        .stack_size(32768)
        .name("app-logic".into());

    let handler = builder.spawn(move || {
        if let Err(e) = run_station() {
            error!("💥 Erro fatal na estação: {:?}", e);
        }
    })?;

    // A main task apenas espera a thread do app (que roda para sempre)
    handler.join().unwrap();
    Ok(())
}

// ─── Lógica da Estação ───────────────────────────────────────────────────────

fn run_station() -> Result<()> {
    let peripherals = Peripherals::take().unwrap();
    let sysloop     = EspSystemEventLoop::take()?;

    // ── WiFi ──────────────────────────────────────────────────────────────────
    let esp_wifi = EspWifi::new(peripherals.modem, sysloop.clone(), None)?;
    let mut wifi = BlockingWifi::wrap(esp_wifi, sysloop)?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid:     WIFI_SSID.try_into().context("SSID inválido")?,
        password: WIFI_PASS.try_into().context("Senha inválida")?,
        ..Default::default()
    }))?;

    wifi.start()?;
    
    info!("🔌 Conectando ao WiFi: {}...", WIFI_SSID);
    let mut ok = false;
    for tentativa in 1u32..=5 {
        match wifi.connect() {
            Ok(_) => {
                wifi.wait_netif_up()?;
                info!("✅ WiFi conectado! IP: {}", wifi.wifi().sta_netif().get_ip_info()?.ip);
                ok = true;
                break;
            }
            Err(e) => {
                warn!("⚠️ Tentativa {}/5 falhou: {:?}", tentativa, e);
                thread::sleep(Duration::from_secs(2));
            }
        }
    }
    if !ok { bail!("❌ Falha total no WiFi"); }

    // ── MQTT ──────────────────────────────────────────────────────────────────
    let cfg_mqtt = MqttClientConfiguration {
        client_id: Some(MQTT_ID),
        ..Default::default()
    };

    let (mut mqtt, mut connection) = EspMqttClient::new(MQTT_BROKER, &cfg_mqtt)
        .context("Erro ao conectar no Broker MQTT")?;

    // Thread de eventos obrigatória para manter a conexão viva
    thread::Builder::new()
        .stack_size(32768)
        .name("mqtt-events".into())
        .spawn(move || loop {
            match connection.next() {
                Ok(ev) => info!("🔔 MQTT: {:?}", ev.payload()),
                Err(e) => { 
                    error!("❌ Erro MQTT: {:?}", e); 
                    thread::sleep(Duration::from_secs(1)); 
                }
            }
        })?;

    // ── Sensores ──────────────────────────────────────────────────────────────
    let mut dht_pin = PinDriver::input_output(peripherals.pins.gpio4, Pull::Floating)?;
    
    let i2c = I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio21,
        peripherals.pins.gpio22,
        &I2cConfig::new().baudrate(KiloHertz(100).into()),
    )?;
    
    let mut bmp = BME280::new_primary(i2c);
    let bmp_ok = bmp.init(&mut Ets).is_ok();
    if bmp_ok { info!("🌬️ BME280 detectado!"); } else { warn!("⚠️ BME280 não encontrado."); }

    // ── Loop ──────────────────────────────────────────────────────────────────
    let mut erros = 0;
    loop {
        match dht11_read(&mut Ets, &mut dht_pin) {
            Ok(d) if d.temperature > -10 && d.temperature < 60 => {
                erros = 0;
                let pressao = if bmp_ok { bmp.measure(&mut Ets).ok().map(|m| m.pressure / 100.0) } else { None };
                
                let payload = match pressao {
                    Some(p) => format!(r#"{{"temperatura":{:.1},"umidade":{:.1},"pressao":{:.1},"localizacao":"{}"}}"#, d.temperature, d.relative_humidity, p, LOCALIZACAO),
                    None    => format!(r#"{{"temperatura":{:.1},"umidade":{:.1},"localizacao":"{}"}}"#, d.temperature, d.relative_humidity, LOCALIZACAO),
                };

                if let Err(e) = mqtt.publish(MQTT_TOPIC, QoS::AtLeastOnce, false, payload.as_bytes()) {
                    error!("❌ Falha publish: {:?}", e);
                } else {
                    info!("📤 Enviado: {}", payload);
                }
            }
            _ => {
                erros += 1;
                warn!("⚠️ Erro sensor. Backoff...");
                thread::sleep(Duration::from_secs((2u64.pow(erros.min(4))).min(30)));
                continue;
            }
        }
        thread::sleep(INTERVALO);
    }
}
