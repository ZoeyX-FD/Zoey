use actix_cors::Cors;
use actix_web::http::header;

pub fn cors_middleware() -> Cors {
    Cors::default()
        .allow_any_origin() // More permissive for development
        .allowed_methods(vec!["GET", "POST", "OPTIONS"])
        .allowed_headers(vec![
            header::AUTHORIZATION,
            header::ACCEPT,
            header::CONTENT_TYPE,
        ])
        .supports_credentials()
        .max_age(3600) // 1 hour
}