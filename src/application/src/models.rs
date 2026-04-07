use serde::{Deserialize, Serialize};
use chrono::NaiveDateTime;
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Leitura {
    pub id: i64,
    pub temperatura: f64,
    pub umidade: f64,
    pub pressao: Option<f64>,
    pub localizacao: Option<String>,
    pub timestamp: NaiveDateTime,
}

#[derive(Debug, Deserialize)]
pub struct NovaLeitura {
    pub temperatura: f64,
    pub umidade: f64,
    #[serde(default)]
    pub pressao: Option<f64>,
    #[serde(default)]
    pub localizacao: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Estatisticas {
    pub media_temp: f64,
    pub media_umid: f64,
    pub min_temp: f64,
    pub max_temp: f64,
}
