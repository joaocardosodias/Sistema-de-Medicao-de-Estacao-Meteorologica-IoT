use anyhow::{bail, Context, Result};
use bme280::i2c::BME280;
use dht_sensor::dht11::blocking::read as dht11_read;
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
use std::{thread, time::Duration};

macro_rules! cfg_str {
    ($key:literal, $default:literal) => {
        match option_env!($key) {
            Some(v) => v,
            None => $default,
        }
    };
}

const WIFI_SSID: &str = cfg_str!("CONFIG_WIFI_SSID", "[SSID]");
const WIFI_PASS: &str = cfg_str!("CONFIG_WIFI_PASS", "[SENHA]");
const MQTT_BROKER: &str = cfg_str!("CONFIG_MQTT_BROKER_URL", "mqtt://[IP_ADDRESS]");
const MQTT_TOPIC: &str = "esp32/sensores";
const MQTT_ID: &str = "esp32-estacao-meteo";
const LOCALIZACAO: &str = "Lab";
const INTERVALO: Duration = Duration::from_secs(5);

fn main() -> Result<()> {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    unsafe {
        let _ = esp_idf_sys::nvs_flash_init();
    }

    info!("Iniciando Estacao Meteorologica...");

    let builder = thread::Builder::new()
        .stack_size(32768)
        .name("app-logic".into());

    let handler = builder.spawn(move || {
        if let Err(e) = run_station() {
            error!("Erro fatal na estacao: {:?}", e);
        }
    })?;

    handler.join().unwrap();
    Ok(())
}

fn run_station() -> Result<()> {
    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;

    let esp_wifi = EspWifi::new(peripherals.modem, sysloop.clone(), None)?;
    let mut wifi = BlockingWifi::wrap(esp_wifi, sysloop)?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: WIFI_SSID.try_into().context("SSID invalido")?,
        password: WIFI_PASS.try_into().context("Senha invalida")?,
        ..Default::default()
    }))?;

    wifi.start()?;

    info!("Conectando ao WiFi: {}...", WIFI_SSID);
    let mut ok = false;
    for tentativa in 1u32..=5 {
        match wifi.connect() {
            Ok(_) => {
                wifi.wait_netif_up()?;
                info!(
                    "WiFi conectado! IP: {}",
                    wifi.wifi().sta_netif().get_ip_info()?.ip
                );
                ok = true;
                break;
            }
            Err(e) => {
                warn!("Tentativa {}/5 falhou: {:?}", tentativa, e);
                thread::sleep(Duration::from_secs(2));
            }
        }
    }
    if !ok {
        bail!("Falha total no WiFi");
    }

    let cfg_mqtt = MqttClientConfiguration {
        client_id: Some(MQTT_ID),
        ..Default::default()
    };

    let (mut mqtt, mut connection) =
        EspMqttClient::new(MQTT_BROKER, &cfg_mqtt).context("Erro ao conectar no Broker MQTT")?;

    thread::Builder::new()
        .stack_size(32768)
        .name("mqtt-events".into())
        .spawn(move || loop {
            match connection.next() {
                Ok(ev) => info!("MQTT: {:?}", ev.payload()),
                Err(e) => {
                    error!("Erro MQTT: {:?}", e);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        })?;

    let mut dht_pin = PinDriver::input_output(peripherals.pins.gpio4, Pull::Floating)?;

    let i2c = I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio21,
        peripherals.pins.gpio22,
        &I2cConfig::new().baudrate(KiloHertz(100).into()),
    )?;

    let mut bmp = BME280::new_primary(i2c);
    let bmp_ok = bmp.init(&mut Ets).is_ok();
    if bmp_ok {
        info!("BME280 detectado!");
    } else {
        warn!("BME280 nao encontrado.");
    }

    let mut erros = 0;
    loop {
        match dht11_read(&mut Ets, &mut dht_pin) {
            Ok(d) if d.temperature > -10 && d.temperature < 60 => {
                erros = 0;
                let pressao = if bmp_ok {
                    bmp.measure(&mut Ets).ok().map(|m| m.pressure / 100.0)
                } else {
                    None
                };

                let payload = match pressao {
                    Some(p) => format!(
                        r#"{{"temperatura":{:.1},"umidade":{:.1},"pressao":{:.1},"localizacao":"{}"}}"#,
                        d.temperature, d.relative_humidity, p, LOCALIZACAO
                    ),
                    None => format!(
                        r#"{{"temperatura":{:.1},"umidade":{:.1},"localizacao":"{}"}}"#,
                        d.temperature, d.relative_humidity, LOCALIZACAO
                    ),
                };

                if let Err(e) =
                    mqtt.publish(MQTT_TOPIC, QoS::AtLeastOnce, false, payload.as_bytes())
                {
                    error!("Falha publish: {:?}", e);
                } else {
                    info!("Enviado: {}", payload);
                }
            }
            _ => {
                erros += 1;
                warn!("Erro sensor. Backoff...");
                thread::sleep(Duration::from_secs((2u64.pow(erros.min(4))).min(30)));
                continue;
            }
        }
        thread::sleep(INTERVALO);
    }
}
