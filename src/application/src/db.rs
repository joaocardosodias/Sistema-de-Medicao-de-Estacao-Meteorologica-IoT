use crate::models::{Estatisticas, Leitura, NovaLeitura};
use sqlx::{sqlite::SqlitePoolOptions, Result, Row, SqlitePool};

pub async fn init_db(database_url: &str) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .idle_timeout(std::time::Duration::from_secs(5))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                use sqlx::Executor;
                conn.execute("PRAGMA journal_mode=WAL;").await?;
                conn.execute("PRAGMA busy_timeout=5000;").await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Falha ao executar as migrations");

    Ok(pool)
}

pub async fn inserir_leitura(pool: &SqlitePool, nova: &NovaLeitura) -> Result<i64> {
    let rec = sqlx::query(
        "INSERT INTO leituras (temperatura, umidade, pressao, localizacao) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(nova.temperatura)
    .bind(nova.umidade)
    .bind(nova.pressao)
    .bind(&nova.localizacao)
    .execute(pool)
    .await?;
    Ok(rec.last_insert_rowid())
}

pub async fn listar_paginado(pool: &SqlitePool, limit: i64, offset: i64) -> Result<Vec<Leitura>> {
    sqlx::query_as::<_, Leitura>(
        "SELECT id, temperatura, umidade, pressao, localizacao, timestamp FROM leituras ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2"
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn contar_leituras(pool: &SqlitePool) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) as total FROM leituras")
        .fetch_one(pool)
        .await?;
    Ok(row.try_get("total").unwrap_or(0))
}

pub async fn listar_para_grafico(pool: &SqlitePool, limite: i64) -> Result<Vec<Leitura>> {
    sqlx::query_as::<_, Leitura>(
        "SELECT id, temperatura, umidade, pressao, localizacao, timestamp FROM (
            SELECT id, temperatura, umidade, pressao, localizacao, timestamp
            FROM leituras ORDER BY timestamp DESC LIMIT ?1
         ) ORDER BY timestamp ASC",
    )
    .bind(limite)
    .fetch_all(pool)
    .await
}

pub async fn buscar_leitura(pool: &SqlitePool, id: i64) -> Result<Leitura> {
    sqlx::query_as::<_, Leitura>(
        "SELECT id, temperatura, umidade, pressao, localizacao, timestamp FROM leituras WHERE id = ?1"
    )
    .bind(id)
    .fetch_one(pool)
    .await
}

pub async fn atualizar_leitura(pool: &SqlitePool, id: i64, dados: &NovaLeitura) -> Result<u64> {
    let rec = sqlx::query(
        "UPDATE leituras SET temperatura = ?1, umidade = ?2, pressao = ?3, localizacao = ?4 WHERE id = ?5"
    )
    .bind(dados.temperatura)
    .bind(dados.umidade)
    .bind(dados.pressao)
    .bind(&dados.localizacao)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(rec.rows_affected())
}

pub async fn deletar_leitura(pool: &SqlitePool, id: i64) -> Result<u64> {
    let rec = sqlx::query("DELETE FROM leituras WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(rec.rows_affected())
}

pub async fn obter_estatisticas(pool: &SqlitePool) -> Result<Estatisticas> {
    let row = sqlx::query(
        "SELECT AVG(temperatura) as media_temp, AVG(umidade) as media_umid,
                MIN(temperatura) as min_temp, MAX(temperatura) as max_temp FROM leituras",
    )
    .fetch_one(pool)
    .await?;

    Ok(Estatisticas {
        media_temp: row.try_get("media_temp").unwrap_or(0.0),
        media_umid: row.try_get("media_umid").unwrap_or(0.0),
        min_temp: row.try_get("min_temp").unwrap_or(0.0),
        max_temp: row.try_get("max_temp").unwrap_or(0.0),
    })
}
