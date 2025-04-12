use actix_web::{ web, App, HttpServer };
use std::sync::Arc;
use qlira_web_server::config::{ ServerConfig, ConfigManager };
use qlira_web_server::handlers::{
    static_files::serve_static_file,
    php_handler::handle_php,
    javascript_handler::handle_js,
    websocket_handler::websocket_handler,
    get_config,
    update_config,
    reload_config,
};
use qlira_web_server::middleware::logger::Logger;
use qlira_web_server::middleware::error_handler;
use qlira_web_server::server::http::start_http_server;
use futures::future::join;

const CONFIG_PATH: &str = "config/server.toml";

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config_manager: Arc<ConfigManager> = match ConfigManager::new(CONFIG_PATH) {
        Ok(manager) => Arc::new(manager),
        Err(e) => {
            eprintln!("chyba pri incializaci config manageru: {}", e);
            std::process::exit(1);
        }
    };

    println!(
        "zapinam server na {}:{}",
        config_manager.get_config().bind_address,
        config_manager.get_config().port
    );

    // spustime pouze HTTP server
    start_http_server(config_manager.clone()).await
}
