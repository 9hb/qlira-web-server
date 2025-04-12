use actix_web::{ web, App, HttpServer, HttpResponse, Responder, HttpRequest };
use std::sync::Arc;
use crate::config::{ ServerConfig, ConfigManager };
use crate::handlers::{
    static_files::serve_static_file,
    php_handler::handle_php,
    javascript_handler::handle_js,
    get_config,
    update_config,
    reload_config,
};
use std::process::{ Command, Stdio };
use std::io::Read;
use std::path::Path;
use std::time::Duration;
use wait_timeout::ChildExt;

pub async fn start_http_server(config_manager: Arc<ConfigManager>) -> std::io::Result<()> {
    // inicializujeme config manager a nacteme konfiguraci
    let config = config_manager.get_config();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(config_manager.clone()))
            .route("/", web::get().to(index))
            .route("/static/{filename:.*}", web::get().to(serve_static_file))
            .route("/php/{filename:.*}", web::to(handle_php))
            .route("/js/{filename:.*}", web::get().to(handle_js))
            // pridame endpointy pro spravu konfigurace
            .route("/api/config", web::get().to(get_config))
            .route("/api/config", web::post().to(update_config))
            .route("/api/config/reload", web::post().to(reload_config))
            .default_service(web::route().to(handle_404))
    })
        .bind(format!("{}:{}", config.bind_address, config.port))?
        .run().await
}

async fn handle_404(
    req: HttpRequest,
    config_manager: web::Data<Arc<ConfigManager>>
) -> impl Responder {
    let config = config_manager.get_config();

    if config.custom_error_pages {
        if let Some(error_page_path) = config.error_pages.get("404") {
            let full_path = std::path::Path::new(&config.static_root).join(error_page_path);

            if full_path.exists() && full_path.is_file() {
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    return HttpResponse::NotFound().content_type("text/html").body(content);
                }
            }
        }
    }

    // kdyz custom 404 neni nacten nebo neexistuje, vracime internal server 404 template
    return HttpResponse::NotFound()
        .content_type("text/html")
        .body(
            r#"
                <!DOCTYPE html>
                <html>
                <head>
                    <title>404 Not Found</title>
                </head>
                <body>
                    <center><h1>404 Not Found</h1></center>
                    <hr>
                    <center>Qlira/0.1.1</center>
                </body>
                </html>
            "#
        );
}

async fn index(config_manager: web::Data<Arc<ConfigManager>>) -> impl Responder {
    let config = config_manager.get_config();
    let server_dir = std::path::Path::new(&config.server_directory);

    let index_files = ["index.php", "index.html"];
    for file in &index_files {
        let file_path = server_dir.join(file);
        if file_path.exists() && file_path.is_file() {
            if file == &"index.php" {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    // pokud zacina <?php, spustime PHP
                    // jinak vracime HTML jako text
                    if content.trim_start().starts_with("<?php") && config.php_enabled {
                        match
                            execute_php_file(
                                &file_path,
                                &config.php_exe_path,
                                config.php_timeout
                            ).await
                        {
                            Ok(output) => {
                                return HttpResponse::Ok().content_type("text/html").body(output);
                            }
                            Err(e) => {
                                println!("chyba pri zpracovani php souboru: {}", e);
                            }
                        }
                    }
                    // pokud PHP neni povoleno nebo soubor neobsahuje PHP, vracime jako text
                    return HttpResponse::Ok().content_type("text/html").body(content);
                }
            } else if let Ok(content) = std::fs::read_to_string(&file_path) {
                let content_type = mime_guess::from_path(file).first_or_octet_stream();
                return HttpResponse::Ok().content_type(content_type.to_string()).body(content);
            }
        }
    }

    if config.custom_error_pages {
        if let Some(error_page_path) = config.error_pages.get("404") {
            let full_path = std::path::Path::new(&config.static_root).join(error_page_path);
            if full_path.exists() && full_path.is_file() {
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    return HttpResponse::NotFound().content_type("text/html").body(content);
                }
            }
        }
    }

    HttpResponse::NotFound()
        .content_type("text/html")
        .body(
            r#"
                <!DOCTYPE html>
                <html>
                <head>
                    <title>404 Not Found</title>
                </head>
                <body>
                    <center><h1>404 Not Found</h1></center>
                    <hr>
                    <center>Qlira/0.1.1</center>
                </body>
                </html>
            "#
        )
}

// pomocna funkce pro spusteni PHP souboru
async fn execute_php_file(
    file_path: &Path,
    php_exe_path: &str,
    timeout_seconds: u64
) -> Result<String, String> {
    let mut command = Command::new(php_exe_path);

    command.arg(file_path);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|e| format!("chyba pri spusteni PHP: {}", e))?;

    let timeout = Duration::from_secs(timeout_seconds);
    match child.wait_timeout(timeout) {
        Ok(status_opt) => {
            match status_opt {
                Some(status) => {
                    if !status.success() {
                        return Err(format!("chyba pri spusteni PHP: {}", status));
                    }
                }
                None => {
                    child.kill().ok();
                    return Err("chyba: PHP proces prekrocil timeout".to_string());
                }
            }
        }
        Err(e) => {
            return Err(format!("chyba pri cekani na PHP proces: {}", e));
        }
    }

    let mut output = String::new();
    child.stdout
        .unwrap()
        .read_to_string(&mut output)
        .map_err(|e| format!("chyba pri cteni vystupu PHP: {}", e))?;

    Ok(output)
}
