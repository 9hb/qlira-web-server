use actix_web::{ dev::ServiceRequest, dev::ServiceResponse, Error, HttpResponse };
use std::sync::Arc;
use crate::config::ConfigManager;

pub async fn error_handler(
    req: ServiceRequest,
    srv: &dyn Fn(ServiceRequest) -> Result<ServiceResponse, Error>,
    config_manager: Arc<ConfigManager>
) -> Result<ServiceResponse, Error> {
    let response = srv(req);

    if response.is_ok() {
        return response;
    }

    let config = config_manager.get_config();

    if !config.custom_error_pages {
        return response;
    }

    let error_code = response
        .as_ref()
        .err()
        .map(|_| 500)
        .unwrap_or_else(|| response.as_ref().unwrap().status().as_u16());

    if let Some(error_page_path) = config.error_pages.get(&error_code.to_string()) {
        let full_path = std::path::Path::new(&config.static_root).join(error_page_path);

        if full_path.exists() && full_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                let custom_response = HttpResponse::build(
                    actix_web::http::StatusCode::from_u16(error_code).unwrap_or_default()
                )
                    .content_type("text/html")
                    .body(content);

                return Ok(ServiceResponse::new(response.unwrap().into_parts().0, custom_response));
            }
        }
    }

    response
}
