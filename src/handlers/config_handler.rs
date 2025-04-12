use actix_web::{ web, HttpRequest, HttpResponse, Responder };
use serde::{ Deserialize, Serialize };
use std::sync::Arc;
use crate::config::{ ServerConfig, ConfigManager };

#[derive(Deserialize)]
pub struct ConfigUpdateRequest {
    section: String,
    key: String,
    value: String,
}

pub async fn get_config(config_manager: web::Data<Arc<ConfigManager>>) -> impl Responder {
    // ziskat aktualni config
    let config = config_manager.get_config();

    // prevest config do JSON
    match serde_json::to_string_pretty(&config) {
        Ok(json) => { HttpResponse::Ok().content_type("application/json").body(json) }
        Err(_) => { HttpResponse::InternalServerError().body("chyba pri serializaci konfigurace") }
    }
}

pub async fn update_config(
    req: web::Json<ConfigUpdateRequest>,
    config_manager: web::Data<Arc<ConfigManager>>
) -> impl Responder {
    // ziskat kopii configu
    let mut config = config_manager.get_config();

    // updatovat config podle zadanych hodnot
    match update_config_value(&mut config, &req.section, &req.key, &req.value) {
        Ok(_) => {
            // ulozit zmeneny config
            match config.save(&config_manager.get_config_path()) {
                Ok(_) => {
                    // config bude automaticky znovu nacten diky watcheru
                    HttpResponse::Ok().body("config byl uspesne aktualizovan")
                }
                Err(e) =>
                    HttpResponse::InternalServerError().body(
                        format!("chyba pri ukladu konfigurace: {}", e)
                    ),
            }
        }
        Err(e) => {
            HttpResponse::BadRequest().body(format!("chyba pri aktualizaci konfigurace: {}", e))
        }
    }
}

pub async fn reload_config(config_manager: web::Data<Arc<ConfigManager>>) -> impl Responder {
    match config_manager.reload() {
        Ok(_) => HttpResponse::Ok().body("uspesne nacteni konfigurace"),
        Err(e) =>
            HttpResponse::InternalServerError().body(
                format!("chyba pri nacteni konfigurace: {}", e)
            ),
    }
}

// pomocna funkce pro aktualizaci hodnot v konfiguraci
fn update_config_value(
    config: &mut ServerConfig,
    section: &str,
    key: &str,
    value: &str
) -> Result<(), String> {
    match section {
        "server" => {
            match key {
                "port" => {
                    config.port = value.parse::<u16>().map_err(|_| "chybny port".to_string())?;
                }
                "timeout" => {
                    config.timeout = value
                        .parse::<u64>()
                        .map_err(|_| "chybny timeout".to_string())?;
                }
                "max_connections" => {
                    config.max_connections = value
                        .parse::<usize>()
                        .map_err(|_| "chybny max_connections".to_string())?;
                }
                "bind_address" => {
                    config.bind_address = value.to_string();
                }
                _ => {
                    return Err(format!("neplatny nastaveni serveru: {}", key));
                }
            }
        }
        "cache" => {
            match key {
                "cache_enabled" => {
                    config.cache_enabled = value
                        .parse::<bool>()
                        .map_err(|_| "neplatna hodnota pro cache_enabled".to_string())?;
                }
                "default_max_age" => {
                    config.default_max_age = value
                        .parse::<u32>()
                        .map_err(|_| "neplatna hodnota pro default_max_age".to_string())?;
                }
                "extension_cache_time" => {
                    let parts: Vec<&str> = value.split(':').collect();
                    if parts.len() != 2 {
                        return Err(
                            "neplatny format pro extension_cache_time, ocekavany format: ext:seconds".to_string()
                        );
                    }

                    let ext = parts[0].to_string();
                    let seconds = parts[1]
                        .parse::<u32>()
                        .map_err(|_| "neplatna hodnota pro seconds".to_string())?;

                    config.file_extension_cache_times.insert(ext, seconds);
                }
                _ => {
                    return Err(format!("neplatny nastaveni cache: {}", key));
                }
            }
        }
        "development" => {
            match key {
                "development_mode" => {
                    config.development_mode = value
                        .parse::<bool>()
                        .map_err(|_| "neplatna hodnota pro development_mode".to_string())?;
                }
                _ => {
                    return Err(format!("neplatny nastaveni development: {}", key));
                }
            }
        }
        "compression" => {
            match key {
                "enable_compression" => {
                    config.enable_compression = value
                        .parse::<bool>()
                        .map_err(|_| "neplatna hodnota pro enable_compression".to_string())?;
                }
                "min_size_to_compress" => {
                    config.min_size_to_compress = value
                        .parse::<usize>()
                        .map_err(|_| "neplatna hodnota pro min_size_to_compress".to_string())?;
                }
                _ => {
                    return Err(format!("neplatny nastaveni compression: {}", key));
                }
            }
        }
        "php" => {
            match key {
                "php_enabled" => {
                    config.php_enabled = value
                        .parse::<bool>()
                        .map_err(|_| "neplatna hodnota pro php_enabled".to_string())?;
                }
                "php_cgi_path" => {
                    config.php_cgi_path = value.to_string();
                }
                "php_exe_path" => {
                    config.php_exe_path = value.to_string();
                }
                "php_root_dir" => {
                    config.php_root_dir = value.to_string();
                }
                "php_timeout" => {
                    config.php_timeout = value
                        .parse::<u64>()
                        .map_err(|_| "neplatna hodnota pro php_timeout".to_string())?;
                }
                _ => {
                    return Err(format!("neplatny nastaveni php: {}", key));
                }
            }
        }
        "static" => {
            match key {
                "static_root" => {
                    config.static_root = value.to_string();
                }
                "max_file_size" => {
                    config.max_file_size = value
                        .parse::<usize>()
                        .map_err(|_| "neplatna hodnota pro max_file_size".to_string())?;
                }
                _ => {
                    return Err(format!("neplatny nastaveni static souboru: {}", key));
                }
            }
        }
        "javascript" => {
            match key {
                "js_minify" => {
                    config.js_minify = value
                        .parse::<bool>()
                        .map_err(|_| "neplatna hodnota pro js_minify".to_string())?;
                }
                "js_root_dir" => {
                    config.js_root_dir = value.to_string();
                }
                _ => {
                    return Err(format!("neplatny nastaveni javascriptu: {}", key));
                }
            }
        }
        _ => {
            return Err(format!("neplatny nastaveni: {}", section));
        }
    }

    Ok(())
}
