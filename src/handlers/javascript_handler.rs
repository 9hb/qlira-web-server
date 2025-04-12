use actix_web::{ web, HttpResponse, Responder, Error, HttpRequest };
use std::fs;
use std::path::{ Path, PathBuf };
use minify_js::{ minify, TopLevelMode };
use std::time::{ UNIX_EPOCH };
use std::sync::Arc;
use crate::config::ConfigManager;

pub async fn handle_js(
    req: HttpRequest,
    config_manager: web::Data<Arc<ConfigManager>>
) -> impl Responder {
    let config = config_manager.get_config();

    let path = req.path();

    let file_path = path.trim_start_matches("/js/");

    let full_path = PathBuf::from(&config.js_root_dir).join(file_path);

    if !Path::new(&full_path).exists() || !Path::new(&full_path).is_file() {
        return HttpResponse::NotFound().body(format!("JavaScript file not found: {}", file_path));
    }

    match fs::read_to_string(&full_path) {
        Ok(content) => {
            let js_content = if config.development_mode || !config.js_minify {
                // v development modu nebo pokud je minifikace zakazana, vracime puvodni obsah
                content
            } else {
                // minifikujeme
                match minify_javascript(&content) {
                    Ok(min) => min,
                    Err(_) => content, // pokud selze, vracime puvodni obsah
                }
            };

            // ziskame timestamp pro posledni upravu souboru
            let last_modified = get_last_modified(&full_path).unwrap_or_else(|| "".to_string());

            // sestavime response
            let mut builder = HttpResponse::Ok();
            let mut response = builder.content_type("application/javascript");

            // pridame cache headery
            if !config.development_mode && config.cache_enabled {
                // Get extension for cache duration lookup
                let ext = Path::new(file_path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();

                let cache_seconds = config.get_cache_duration(&ext);

                if cache_seconds > 0 {
                    response = response.insert_header((
                        "Cache-Control",
                        format!("public, max-age={}", cache_seconds),
                    ));
                }
            } else {
                // zadny cachovani v development modu
                response = response.insert_header(("Cache-Control", "no-store, max-age=0"));
            }

            // pridame last-modified header
            if !last_modified.is_empty() {
                response = response.insert_header(("Last-Modified", last_modified));
            }

            response.body(js_content)
        }
        Err(_) => {
            HttpResponse::InternalServerError().body(
                format!("Failed to read JavaScript file: {}", file_path)
            )
        }
    }
}

fn minify_javascript(js_code: &str) -> Result<String, String> {
    let mut out = Vec::new();

    let session = minify_js::Session::new();

    match minify(&session, TopLevelMode::Global, js_code.as_bytes(), &mut out) {
        Ok(_) => {
            match String::from_utf8(out) {
                Ok(minified) => Ok(minified),
                Err(_) => Err("Failed to convert minified JS to UTF-8".to_string()),
            }
        }
        Err(e) => Err(format!("Minification error: {:?}", e)),
    }
}

// pomocna funkce pro ziskani casu posledni upravy souboru
fn get_last_modified(path: &PathBuf) -> Option<String> {
    if let Ok(metadata) = fs::metadata(path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                let timestamp = duration.as_secs();
                // prevedeme timestamp na UTC a potom formatujeme na RFC 1123
                let dt = chrono::DateTime::<chrono::Utc>::from_utc(
                    chrono::NaiveDateTime::from_timestamp_opt(timestamp as i64, 0)?,
                    chrono::Utc
                );
                return Some(dt.format("%a, %d %b %Y %H:%M:%S GMT").to_string());
            }
        }
    }
    None
}
