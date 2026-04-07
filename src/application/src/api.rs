use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_ws::AggregatedMessage;
use futures_util::StreamExt as _;
use sqlx::SqlitePool;
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::db;
use crate::models::NovaLeitura;

// ── Query structs ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct PaginacaoQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
pub struct LimiteQuery {
    pub limit: Option<i64>,
}

// ── REST handlers ─────────────────────────────────────────

pub async fn listar(pool: web::Data<SqlitePool>, query: web::Query<PaginacaoQuery>) -> HttpResponse {
    let limit  = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0).max(0);
    let dados  = db::listar_paginado(&pool, limit, offset).await.unwrap_or_default();
    let total  = db::contar_leituras(&pool).await.unwrap_or(0);

    HttpResponse::Ok().json(serde_json::json!({
        "dados": dados,
        "total": total,
        "limit": limit,
        "offset": offset,
    }))
}

pub async fn criar(pool: web::Data<SqlitePool>, nova: web::Json<NovaLeitura>) -> HttpResponse {
    match db::inserir_leitura(&pool, &nova).await {
        Ok(id) => HttpResponse::Created().json(serde_json::json!({ "id": id, "status": "criado" })),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

pub async fn detalhe(pool: web::Data<SqlitePool>, path: web::Path<i64>) -> HttpResponse {
    match db::buscar_leitura(&pool, path.into_inner()).await {
        Ok(l) => HttpResponse::Ok().json(l),
        Err(_) => HttpResponse::NotFound().json(serde_json::json!({ "erro": "Não encontrado" })),
    }
}

pub async fn atualizar(
    pool: web::Data<SqlitePool>,
    path: web::Path<i64>,
    dados: web::Json<NovaLeitura>,
) -> HttpResponse {
    match db::atualizar_leitura(&pool, path.into_inner(), &dados).await {
        Ok(n) if n > 0 => HttpResponse::Ok().json(serde_json::json!({ "status": "atualizado" })),
        Ok(_) => HttpResponse::NotFound().json(serde_json::json!({ "erro": "Não encontrado" })),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

pub async fn deletar(pool: web::Data<SqlitePool>, path: web::Path<i64>) -> impl Responder {
    match db::deletar_leitura(&pool, path.into_inner()).await {
        Ok(n) if n > 0 => HttpResponse::Ok().json(serde_json::json!({ "status": "deletado" })),
        Ok(_) => HttpResponse::NotFound().json(serde_json::json!({ "erro": "Não encontrado" })),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

pub async fn estatisticas(pool: web::Data<SqlitePool>) -> impl Responder {
    match db::obter_estatisticas(&pool).await {
        Ok(s) => HttpResponse::Ok().json(s),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn grafico(pool: web::Data<SqlitePool>, query: web::Query<LimiteQuery>) -> HttpResponse {
    let limite = query.limit.unwrap_or(20).min(100);
    match db::listar_para_grafico(&pool, limite).await {
        Ok(l) => HttpResponse::Ok().json(l),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

// ── WebSocket handler ─────────────────────────────────────

/// GET /ws
/// Mantém uma conexão WebSocket aberta. Sempre que o MQTT recebe uma
/// nova leitura, o servidor empurra o JSON para todos os clients conectados.
pub async fn ws_handler(
    req: HttpRequest,
    body: web::Payload,
    tx: web::Data<broadcast::Sender<String>>,
) -> Result<HttpResponse, actix_web::Error> {
    let (response, mut session, stream) = actix_ws::handle(&req, body)?;

    // Subscreve ao canal de broadcast interno
    let mut rx = tx.subscribe();

    // Agrupa mensagens fragmentadas do WebSocket
    let mut stream = stream
        .aggregate_continuations()
        .max_continuation_size(65_536);

    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                // Nova leitura chegou via MQTT → encaminha ao browser
                Ok(json) = rx.recv() => {
                    if session.text(json).await.is_err() {
                        break;
                    }
                }
                // Mensagem vinda do browser (ping / close)
                Some(msg) = stream.next() => {
                    match msg {
                        Ok(AggregatedMessage::Ping(b)) => {
                            if session.pong(&b).await.is_err() { break; }
                        }
                        Ok(AggregatedMessage::Close(_)) | Err(_) => break,
                        _ => {}
                    }
                }
                else => break,
            }
        }
        let _ = session.close(None).await;
    });

    Ok(response)
}

// ── Registro de rotas ─────────────────────────────────────

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        // WebSocket — fora do scope /api, no topo
        .service(web::resource("/ws").route(web::get().to(ws_handler)))
        .service(
            web::scope("/api")
                .service(
                    web::resource("/leituras")
                        .route(web::get().to(listar))
                        .route(web::post().to(criar)),
                )
                .service(
                    web::resource("/leituras/{id}")
                        .route(web::get().to(detalhe))
                        .route(web::put().to(atualizar))
                        .route(web::delete().to(deletar)),
                )
                .service(web::resource("/estatisticas").route(web::get().to(estatisticas)))
                .service(web::resource("/grafico").route(web::get().to(grafico))),
        );
}
