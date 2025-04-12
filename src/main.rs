use std::sync::Arc;
use qlira_web_server::config::{ ConfigManager };
use qlira_web_server::server::http::start_http_server;

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
