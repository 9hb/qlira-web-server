use serde::{ Deserialize, Serialize };
use std::fs;
use std::sync::RwLock;
use std::path::Path;
use std::time::{ Duration, Instant };
use std::collections::HashMap;
use std::thread;
use std::sync::mpsc::channel;
use notify::{ Watcher, RecursiveMode, recommended_watcher };

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    // nastaveni serveru
    pub server_directory: String,
    pub port: u16,
    pub timeout: u64,
    pub max_connections: usize,
    pub bind_address: String,

    // nastaveni cachovani
    pub cache_enabled: bool,
    pub default_max_age: u32,
    pub file_extension_cache_times: HashMap<String, u32>,

    // development mode
    pub development_mode: bool,

    // nastaveni komprese
    pub enable_compression: bool,
    pub min_size_to_compress: usize,

    // nastaveni statickych souboru
    pub static_root: String,
    pub max_file_size: usize,

    // nastaveni JavaScriptu
    pub js_minify: bool,
    pub js_root_dir: String,

    // nastaveni WebSockets
    pub enable_websockets: bool,
    pub websocket_path: String,
    pub websocket_max_connections: usize,
    pub websocket_timeout: u64,

    // nastaveni vlastnich error stranek
    pub custom_error_pages: bool,
    pub error_pages: HashMap<String, String>,

    // nastaveni PHP
    pub php_enabled: bool,
    pub php_cgi_path: String,
    pub php_exe_path: String,
    pub php_root_dir: String,
    pub php_timeout: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        let mut file_extension_cache_times = HashMap::new();

        //
        // defaultni casy cachovani pro ruzne typy souboru
        //

        // 1 hodina
        file_extension_cache_times.insert("html".to_string(), 3600);
        file_extension_cache_times.insert("htm".to_string(), 3600);
        // 1 tyden
        file_extension_cache_times.insert("css".to_string(), 604800);
        file_extension_cache_times.insert("js".to_string(), 604800);
        // 1 mesic
        file_extension_cache_times.insert("woff2".to_string(), 2592000);
        file_extension_cache_times.insert("woff".to_string(), 2592000);
        file_extension_cache_times.insert("ttf".to_string(), 2592000);
        file_extension_cache_times.insert("eot".to_string(), 2592000);
        file_extension_cache_times.insert("otf".to_string(), 2592000);
        // 1 rok
        file_extension_cache_times.insert("jpg".to_string(), 31536000);
        file_extension_cache_times.insert("jpeg".to_string(), 31536000);
        file_extension_cache_times.insert("png".to_string(), 31536000);
        file_extension_cache_times.insert("gif".to_string(), 31536000);
        file_extension_cache_times.insert("ico".to_string(), 31536000);
        file_extension_cache_times.insert("svg".to_string(), 31536000);
        file_extension_cache_times.insert("webp".to_string(), 31536000);

        let mut error_pages = HashMap::new();
        let http_error_codes = [
            400, 401, 402, 403, 404, 405, 406, 407, 408, 409, 410, 411, 412, 413, 414, 415, 416, 417,
            418, 421, 422, 423, 424, 425, 426, 428, 429, 431, 451, 500, 501, 502, 503, 504, 505, 506,
            507, 508, 510, 511,
        ];

        for &code in http_error_codes.iter() {
            error_pages.insert(code.to_string(), format!("errors/{}.html", code));
        }

        ServerConfig {
            server_directory: "server".to_string(),
            port: 8080,
            timeout: 30,
            max_connections: 1000,
            bind_address: "0.0.0.0".to_string(),

            cache_enabled: true,
            // 1 den
            default_max_age: 86400,
            file_extension_cache_times,

            development_mode: false,

            enable_compression: true,
            // 1KB
            min_size_to_compress: 1024,

            static_root: "static".to_string(),
            // 10MB
            max_file_size: 10 * 1024 * 1024,

            js_minify: true,
            js_root_dir: "static/js".to_string(),

            // defaultni WebSocket nastaveni
            enable_websockets: false,
            websocket_path: "/ws".to_string(),
            websocket_max_connections: 1000,
            websocket_timeout: 60,

            // defaultni nastaveni error stranek
            custom_error_pages: false,
            error_pages,

            php_enabled: true,
            php_cgi_path: "php/php-cgi.exe".to_string(),
            php_exe_path: "php/php.exe".to_string(), // Výchozí cesta k PHP.exe
            php_root_dir: "web".to_string(),
            php_timeout: 30,
        }
    }
}

impl ServerConfig {
    pub fn load(config_path: &str) -> Result<Self, String> {
        if !Path::new(config_path).exists() {
            let default_config = Self::default();
            default_config.save(config_path)?;
            return Ok(default_config);
        }

        let content = fs
            ::read_to_string(config_path)
            .map_err(|e| format!("chyba pri cteni configu: {}", e))?;

        let config: ServerConfig = toml
            ::from_str(&content)
            .map_err(|e| format!("chyba pri parsovani configu: {}", e))?;

        Ok(config)
    }

    pub fn save(&self, config_path: &str) -> Result<(), String> {
        // Ensure the directory exists
        if let Some(parent_dir) = Path::new(config_path).parent() {
            if !parent_dir.exists() {
                fs
                    ::create_dir_all(parent_dir)
                    .map_err(|e| format!("chyba pri vytvareni adresare configu: {}", e))?;
            }
        }

        let content = toml
            ::to_string_pretty(self)
            .map_err(|e| format!("chyba pri serializaci configu: {}", e))?;

        fs
            ::write(config_path, content)
            .map_err(|e| format!("chyba pri zapisovani configu: {}", e))?;

        Ok(())
    }

    pub fn get_cache_duration(&self, extension: &str) -> u32 {
        if !self.cache_enabled || self.development_mode {
            return 0; // zadny cachovani v development modu
        }

        self.file_extension_cache_times.get(extension).copied().unwrap_or(self.default_max_age)
    }
}

pub struct ConfigManager {
    config: RwLock<ServerConfig>,
    config_path: String,
    last_reload: RwLock<Instant>,
}

impl ConfigManager {
    pub fn new(config_path: &str) -> Result<Self, String> {
        let config = ServerConfig::load(config_path)?;

        let manager = ConfigManager {
            config: RwLock::new(config),
            config_path: config_path.to_string(),
            last_reload: RwLock::new(Instant::now()),
        };

        // file watcher -> pro sledovani zmen v configu a automaticke reloadovani
        manager.setup_watcher()?;

        Ok(manager)
    }

    pub fn get_config(&self) -> ServerConfig {
        self.config.read().unwrap().clone()
    }

    pub fn reload(&self) -> Result<(), String> {
        let new_config = ServerConfig::load(&self.config_path)?;

        // aktualizace konfigurace
        let mut config = self.config.write().unwrap();

        // aktualizace casu posledniho reloadingu
        let mut last_reload = self.last_reload.write().unwrap();
        *last_reload = Instant::now();

        println!("config nacten z {}", self.config_path);

        Ok(())
    }

    pub fn get_config_path(&self) -> String {
        self.config_path.clone()
    }

    fn setup_watcher(&self) -> Result<(), String> {
        let config_path = self.config_path.clone();
        let config_manager = ConfigManager {
            config: RwLock::new(self.config.read().unwrap().clone()),
            config_path: self.config_path.clone(),
            last_reload: RwLock::new(*self.last_reload.read().unwrap()),
        };

        thread::spawn(move || {
            let (tx, rx) = channel();

            let mut watcher = match
                recommended_watcher(move |res| {
                    if let Ok(event) = res {
                        let _ = tx.send(event);
                    }
                })
            {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("chyba - nelze vytvorit watcher: {}", e);
                    return false;
                }
            };

            if let Err(e) = watcher.watch(Path::new(&config_path), RecursiveMode::NonRecursive) {
                eprintln!("chyba - nelze sledovat config {}", e);
                return false;
            }

            println!("sledovani zmen v configu: {}", config_path);

            loop {
                match rx.recv() {
                    Ok(_) => {
                        // soubor configu byl zmenen, reloaduju
                        if let Err(e) = config_manager.reload() {
                            eprintln!("chyba pri reloadovani configu: {}", e);
                        }
                    }
                    Err(e) => eprintln!("chyba pri prijmu udalosti (z file watcheru): {}", e),
                }
            }
        });

        Ok(())
    }
}
