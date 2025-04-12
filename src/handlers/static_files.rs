use actix_web::{ web, HttpRequest, HttpResponse, Responder };
use std::fs;
use std::io;
use std::path::{ Path, PathBuf };
use std::time::{ UNIX_EPOCH };
use mime_guess::from_path;
use std::io::Read;
use std::sync::Arc;
use actix_web::http::header;
use ring::digest::{ Context, SHA256 };
use brotli::CompressorReader;
use flate2::read::{ GzEncoder, DeflateEncoder };
use flate2::Compression;
use std::collections::HashMap;
use crate::config::ConfigManager;

// mapovani pripony souboru na dobu trvani cache (v sekundach)
lazy_static::lazy_static! {
    static ref CACHE_POLICY: HashMap<&'static str, u32> = {
        let mut m = HashMap::new();
        // casto menici se
        m.insert("html", 3600); // 1 hodina
        m.insert("htm", 3600);

        // stabilni
        m.insert("css", 604800); // 1 tyden
        m.insert("js", 604800);
        m.insert("woff2", 2592000); // 1 mesic
        m.insert("woff", 2592000);
        m.insert("ttf", 2592000);
        m.insert("eot", 2592000);
        m.insert("otf", 2592000);
        
        // staticky
        m.insert("jpg", 31536000); // 1 rok
        m.insert("jpeg", 31536000);
        m.insert("png", 31536000);
        m.insert("gif", 31536000);
        m.insert("ico", 31536000);
        m.insert("svg", 31536000);
        m.insert("webp", 31536000);
        
        m
    };
}

pub async fn serve_static_file(
    req: HttpRequest,
    config_manager: web::Data<Arc<ConfigManager>>
) -> impl Responder {
    let config = config_manager.get_config();

    let filename = req.match_info().get("filename").unwrap_or("");

    // normalizovani cesty, abychom preventnuli directory traversal a ziskali spravnou cestu
    let path = match normalize_path(filename) {
        Ok(p) => p,
        Err(_) => {
            return HttpResponse::BadRequest().body("neplatna cesta");
        }
    };

    let full_path = PathBuf::from(&config.static_root).join(&path);

    if !full_path.exists() || !full_path.is_file() {
        return HttpResponse::NotFound().body(format!("soubor nenalezen: {}", path));
    }

    // ziskame metadata
    let metadata = match fs::metadata(&full_path) {
        Ok(m) => m,
        Err(_) => {
            return HttpResponse::InternalServerError().body("chyba pri ziskavani metadat souboru");
        }
    };

    // jestli je velikost souboru vetsi nez max_file_size, vracime chybu
    if (metadata.len() as usize) > config.max_file_size {
        return HttpResponse::PayloadTooLarge().body("soubor je prilis velky");
    }

    // pokud je development mode, vracime soubor bez cachovani
    if config.development_mode {
        match fs::read(&full_path) {
            Ok(content) => {
                HttpResponse::Ok()
                    .content_type(get_content_type(&path))
                    .insert_header((header::CACHE_CONTROL, "no-store, max-age=0"))
                    .body(content)
            }
            Err(_) => { HttpResponse::InternalServerError().body("chyba pri cteni souboru") }
        }
    } else {
        // vygenerujeme ETag pro soubor
        let etag = match generate_etag(&full_path) {
            Ok(tag) => tag,
            Err(_) => String::new(),
        };

        // ziskame timestamp pro posledni upravu souboru
        let last_modified = match get_last_modified(&full_path) {
            Some(time) => time,
            None => String::new(),
        };

        // handle podminenych pozadavku (If-None-Match a If-Modified-Since)
        if !etag.is_empty() {
            if let Some(if_none_match) = req.headers().get(header::IF_NONE_MATCH) {
                if let Ok(if_none_match_str) = if_none_match.to_str() {
                    if if_none_match_str == etag {
                        return HttpResponse::NotModified()
                            .insert_header((header::ETAG, etag))
                            .finish();
                    }
                }
            }
        }

        if !last_modified.is_empty() {
            if let Some(if_modified_since) = req.headers().get(header::IF_MODIFIED_SINCE) {
                if let Ok(if_modified_since_str) = if_modified_since.to_str() {
                    if if_modified_since_str == last_modified {
                        return HttpResponse::NotModified()
                            .insert_header((header::LAST_MODIFIED, last_modified))
                            .finish();
                    }
                }
            }
        }

        let content = match fs::read(&full_path) {
            Ok(c) => c,
            Err(_) => {
                return HttpResponse::InternalServerError().body("chyba pri cteni souboru");
            }
        };

        let content_type = get_content_type(&path);

        // rozhodneme, jestli je obsah comprimovatelny
        let should_compress =
            config.enable_compression &&
            is_compressible(&content_type) &&
            content.len() > config.min_size_to_compress;

        // ziskame hodnotu Accept-Encoding z requestu pro porovnani s podporovanymi typy komprese
        let accepted_encodings = match req.headers().get(header::ACCEPT_ENCODING) {
            Some(val) =>
                match val.to_str() {
                    Ok(encodings) => encodings.to_lowercase(),
                    Err(_) => String::new(),
                }
            None => String::new(),
        };

        // aplikujeme kompresi podle toho, co je podporovano
        let (body, encoding) = if should_compress {
            if accepted_encodings.contains("br") {
                match compress_brotli(&content) {
                    Ok(compressed) => (compressed, Some("br")),
                    Err(_) => (content, None),
                }
            } else if accepted_encodings.contains("gzip") {
                match compress_gzip(&content) {
                    Ok(compressed) => (compressed, Some("gzip")),
                    Err(_) => (content, None),
                }
            } else if accepted_encodings.contains("deflate") {
                match compress_deflate(&content) {
                    Ok(compressed) => (compressed, Some("deflate")),
                    Err(_) => (content, None),
                }
            } else {
                (content, None)
            }
        } else {
            (content, None)
        };

        // ziskame priponu souboru pro urceni typu obsahu
        let ext = Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        let cache_seconds = config.get_cache_duration(&ext);

        // sestavime response na zaklade content type
        let mut builder = HttpResponse::Ok();
        let mut response = builder.content_type(content_type);

        if cache_seconds > 0 {
            response = response.insert_header((
                header::CACHE_CONTROL,
                format!("public, max-age={}", cache_seconds),
            ));
        } else {
            response = response.insert_header((header::CACHE_CONTROL, "no-store, max-age=0"));
        }

        if !etag.is_empty() {
            response = response.insert_header((header::ETAG, etag));
        }

        if !last_modified.is_empty() {
            response = response.insert_header((header::LAST_MODIFIED, last_modified));
        }

        // pridame content length header
        if let Some(enc) = encoding {
            response = response.insert_header((header::CONTENT_ENCODING, enc));
        }

        response.body(body)
    }
}

// kontrola jestli cesta neobsahuje ".." nebo "..\"
fn normalize_path(path: &str) -> Result<String, io::Error> {
    let path = path.replace('\\', "/");
    let normalized = Path::new(&path)
        .components()
        .fold(PathBuf::new(), |mut result, p| {
            match p {
                std::path::Component::Normal(x) => result.push(x),
                // preskocime vsechny ostatni typy komponent (jako ParentDir, ktery by mohl byt pouzit pro directory traversal)
                _ => {}
            }
            result
        });

    match normalized.to_str() {
        Some(s) => Ok(s.to_string()),
        None => Err(io::Error::new(io::ErrorKind::InvalidInput, "chybna cesta")),
    }
}

fn get_content_type(path: &str) -> String {
    let mime = from_path(path).first_or_octet_stream();
    mime.to_string()
}

fn generate_etag(path: &Path) -> Result<String, io::Error> {
    let mut file = fs::File::open(path)?;
    let mut context = Context::new(&SHA256);
    let mut buffer = [0; 4096];

    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
    }

    let digest = context.finish();
    let etag = format!("\"{}\"", hex::encode(digest.as_ref()));
    Ok(etag)
}

fn get_last_modified(path: &Path) -> Option<String> {
    if let Ok(metadata) = fs::metadata(path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                let timestamp = duration.as_secs();
                // prevedeme timestamp na UTC a formatneme ho do spravneho formatu
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

fn is_compressible(content_type: &str) -> bool {
    let compressible_types = [
        "text/",
        "application/javascript",
        "application/json",
        "application/xml",
        "application/xhtml+xml",
        "image/svg+xml",
        "application/wasm",
    ];

    compressible_types.iter().any(|&t| content_type.starts_with(t))
}

fn compress_brotli(content: &[u8]) -> Result<Vec<u8>, io::Error> {
    let mut reader = CompressorReader::new(content, 4096, 4, 22);
    let mut compressed = Vec::new();
    reader.read_to_end(&mut compressed)?;
    Ok(compressed)
}

fn compress_gzip(content: &[u8]) -> Result<Vec<u8>, io::Error> {
    let mut encoder = GzEncoder::new(content, Compression::default());
    let mut compressed = Vec::new();
    encoder.read_to_end(&mut compressed)?;
    Ok(compressed)
}

fn compress_deflate(content: &[u8]) -> Result<Vec<u8>, io::Error> {
    let mut encoder = DeflateEncoder::new(content, Compression::default());
    let mut compressed = Vec::new();
    encoder.read_to_end(&mut compressed)?;
    Ok(compressed)
}
