use actix_web::{get, web, HttpResponse};
use crate::MultiAgentSystem;

#[get("/health")]
pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

#[get("/market-data")]
pub async fn market_overview(system: web::Data<MultiAgentSystem>) -> HttpResponse {
    match system.get_market_data().await {
        Ok(data) => HttpResponse::Ok().json(data),
        Err(e) => {
            println!("Error getting market data: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch market data"
            }))
        }
    }
}

#[get("/technical-analysis")]
pub async fn technical_analysis(system: web::Data<MultiAgentSystem>) -> HttpResponse {
    match system.get_technical_analysis().await {
        Ok(data) => HttpResponse::Ok().json(data),
        Err(e) => {
            println!("Error getting technical analysis: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch technical analysis"
            }))
        }
    }
}

#[get("/topic-analysis")]
pub async fn topic_analysis(system: web::Data<MultiAgentSystem>) -> HttpResponse {
    match system.get_topic_analysis().await {
        Ok(data) => HttpResponse::Ok().json(data),
        Err(e) => {
            println!("Error getting topic analysis: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch topic analysis"
            }))
        }
    }
}

#[get("/sentiment-analysis")]
pub async fn sentiment_analysis(system: web::Data<MultiAgentSystem>) -> HttpResponse {
    match system.get_sentiment_analysis().await {
        Ok(data) => HttpResponse::Ok().json(data),
        Err(e) => {
            println!("Error getting sentiment analysis: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch sentiment analysis"
            }))
        }
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(health_check)
        .service(market_overview)
        .service(technical_analysis)
        .service(topic_analysis)
        .service(sentiment_analysis);
}