use actix_web::{ web, HttpRequest, HttpResponse, Responder };
use actix_web::http::StatusCode;
use std::process::{ Command, Stdio };
use std::io::{ Write, Read };
use std::path::{ Path, PathBuf };
use std::collections::HashMap;
use std::time::Duration;
use futures::StreamExt;
use wait_timeout::ChildExt;
use std::sync::Arc;
use std::fs;
use crate::config::ConfigManager;

const MAX_REQUEST_SIZE: usize = 8 * 1024 * 1024; // 8MB

pub async fn handle_php(
    req: HttpRequest,
    mut payload: web::Payload,
    config_manager: web::Data<Arc<ConfigManager>>
) -> impl Responder {
    let config = config_manager.get_config();

    if !config.php_enabled {
        return HttpResponse::NotFound().body("PHP processing is disabled");
    }

    // extrahujeme cestu z requestu
    let path = req.path();
    let script_path = path.trim_start_matches("/php/");

    // sestrojime celou cestu k PHP scriptu
    let script_filename = if script_path.is_empty() {
        format!("{}/index.php", config.php_root_dir)
    } else {
        format!("{}/{}", config.php_root_dir, script_path)
    };

    // zkontrolujeme jestli PHP script existuje
    let script_path = PathBuf::from(&script_filename);
    if !script_path.exists() || !script_path.is_file() {
        return HttpResponse::NotFound().body(format!("PHP script not found: {}", script_filename));
    }

    let uses_php_tags = check_php_tags(&script_path);

    // extrahujeme query string
    let query_string = req.query_string();

    // sestavime environment variables pro PHP-CGI
    let mut env_vars = HashMap::new();
    env_vars.insert("SCRIPT_FILENAME".to_string(), script_filename.clone());
    env_vars.insert("SCRIPT_NAME".to_string(), format!("/php/{}", script_path.display()));
    env_vars.insert("REQUEST_METHOD".to_string(), req.method().to_string());
    env_vars.insert("QUERY_STRING".to_string(), query_string.to_string());
    env_vars.insert("REQUEST_URI".to_string(), req.uri().to_string());
    env_vars.insert("SERVER_NAME".to_string(), req.connection_info().host().to_string());
    env_vars.insert("SERVER_PROTOCOL".to_string(), "HTTP/1.1".to_string());
    env_vars.insert("GATEWAY_INTERFACE".to_string(), "CGI/1.1".to_string());

    // pridame environment variables pro debugovani, kdyz je zapnuty development mode
    if config.development_mode {
        env_vars.insert("DEVELOPMENT_MODE".to_string(), "1".to_string());
    }

    // pridame environment variables pro HTTP headers
    for (header_name, header_value) in req.headers() {
        let header_env_name = format!(
            "HTTP_{}",
            header_name.to_string().replace("-", "_").to_uppercase()
        );
        if let Ok(value_str) = header_value.to_str() {
            env_vars.insert(header_env_name, value_str.to_string());
        }
    }

    // pridame environment variables pro POST, PUT, atd...
    let mut request_body = Vec::new();
    while let Some(chunk) = payload.next().await {
        match chunk {
            Ok(chunk) => {
                request_body.extend_from_slice(&chunk);
                // kontrola velikosti requestu
                if request_body.len() > MAX_REQUEST_SIZE {
                    return HttpResponse::PayloadTooLarge().body("request body je prilis velky");
                }
            }
            Err(_) => {
                return HttpResponse::BadRequest().body("chyba pri zpracovani requestu");
            }
        }
    }

    // pridame CONTENT_LENGTH a CONTENT_TYPE do environment variables
    if !request_body.is_empty() {
        env_vars.insert("CONTENT_LENGTH".to_string(), request_body.len().to_string());

        // Try to determine content type from headers
        if let Some(content_type) = req.headers().get("content-type") {
            if let Ok(content_type_str) = content_type.to_str() {
                env_vars.insert("CONTENT_TYPE".to_string(), content_type_str.to_string());
            }
        }
    }

    if uses_php_tags {
        // Pro soubory začínající <?php použijeme php.exe
        let php_response = execute_php_exe(
            &script_filename,
            env_vars,
            &request_body,
            &config.php_exe_path,
            config.php_timeout
        ).await;

        match php_response {
            Ok(response) => { HttpResponse::Ok().content_type("text/html").body(response) }
            Err(err) => { HttpResponse::InternalServerError().body(format!("php error: {}", err)) }
        }
    } else {
        // Pro ostatní PHP soubory použijeme CGI
        let php_response = execute_php_cgi(
            &script_filename,
            env_vars,
            &request_body,
            &config.php_cgi_path,
            config.php_timeout
        ).await;

        match php_response {
            Ok(response) => {
                // parsujeme PHP response (header a body)
                parse_php_response(response)
            }
            Err(err) => { HttpResponse::InternalServerError().body(format!("php error: {}", err)) }
        }
    }
}

// Funkce ke kontrole, zda soubor začíná značkou <?php
fn check_php_tags(script_path: &Path) -> bool {
    if let Ok(content) = fs::read_to_string(script_path) {
        content.trim_start().starts_with("<?php")
    } else {
        false
    }
}

async fn execute_php_cgi(
    script_filename: &str,
    env_vars: HashMap<String, String>,
    request_body: &[u8],
    php_cgi_path: &str,
    timeout_seconds: u64
) -> Result<String, String> {
    // vytvorime PHP-CGI proces pro zpracovani PHP scriptu
    let mut command = Command::new(php_cgi_path);

    // nastavime vlastnosti pro PHP-CGI
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.current_dir(Path::new(script_filename).parent().unwrap_or(Path::new(".")));

    // nastavime environment variables
    for (key, value) in env_vars {
        command.env(key, value);
    }

    // spawnneme process az po nastaveni environment variables
    let mut child = command.spawn().map_err(|e| format!("chyba pri spousteni PHP-CGI: {}", e))?;

    if !request_body.is_empty() {
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(request_body)
                .map_err(|e| format!("chyba pri zapisovani do stdin PHP-CGI: {}", e))?;
        }
    }

    // nastvime timeout pro PHP-CGI proces
    let timeout = Duration::from_secs(timeout_seconds);
    match child.wait_timeout(timeout) {
        Ok(status_opt) => {
            match status_opt {
                Some(status) => {
                    if !status.success() {
                        return Err(format!("chyba pri behu PHP-CGI: {}", status));
                    }
                }
                None => {
                    // Process timeout
                    child.kill().ok();
                    return Err("chyba: PHP-CGI proces prekrocil timeout".to_string());
                }
            }
        }
        Err(e) => {
            return Err(format!("chyba pri cekani na PHP-CGI: {}", e));
        }
    }

    // sebereme output
    let mut output = String::new();
    child.stdout
        .unwrap()
        .read_to_string(&mut output)
        .map_err(|e| format!("chyba pri cteni PHP-CGI stdout: {}", e))?;

    // sebereme error output
    let mut error_output = String::new();
    child.stderr
        .unwrap()
        .read_to_string(&mut error_output)
        .map_err(|e| format!("chyba pri cteni PHP-CGI stderr: {}", e))?;

    if !error_output.is_empty() {
        println!("chyba pri behu PHP-CGI: {}", error_output);
    }

    Ok(output)
}

// Nová funkce pro spuštění PHP pomocí php.exe
async fn execute_php_exe(
    script_filename: &str,
    env_vars: HashMap<String, String>,
    request_body: &[u8],
    php_exe_path: &str,
    timeout_seconds: u64
) -> Result<String, String> {
    // vytvorime PHP proces pro zpracovani PHP scriptu
    let mut command = Command::new(php_exe_path);

    // nastavime parametry pro php.exe
    command.arg(script_filename);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.current_dir(Path::new(script_filename).parent().unwrap_or(Path::new(".")));

    // nastavime environment variables
    for (key, value) in env_vars {
        command.env(key, value);
    }

    // spawnneme process
    let mut child = command.spawn().map_err(|e| format!("chyba pri spousteni PHP: {}", e))?;

    if !request_body.is_empty() {
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(request_body)
                .map_err(|e| format!("chyba pri zapisovani do stdin PHP: {}", e))?;
        }
    }

    // nastvime timeout pro PHP proces
    let timeout = Duration::from_secs(timeout_seconds);
    match child.wait_timeout(timeout) {
        Ok(status_opt) => {
            match status_opt {
                Some(status) => {
                    if !status.success() {
                        return Err(format!("chyba pri behu PHP: {}", status));
                    }
                }
                None => {
                    // Process timeout
                    child.kill().ok();
                    return Err("chyba: PHP proces prekrocil timeout".to_string());
                }
            }
        }
        Err(e) => {
            return Err(format!("chyba pri cekani na PHP: {}", e));
        }
    }

    // sebereme output
    let mut output = String::new();
    child.stdout
        .unwrap()
        .read_to_string(&mut output)
        .map_err(|e| format!("chyba pri cteni PHP stdout: {}", e))?;

    // sebereme error output
    let mut error_output = String::new();
    child.stderr
        .unwrap()
        .read_to_string(&mut error_output)
        .map_err(|e| format!("chyba pri cteni PHP stderr: {}", e))?;

    if !error_output.is_empty() {
        println!("chyba pri behu PHP: {}", error_output);
    }

    Ok(output)
}

fn parse_php_response(response: String) -> HttpResponse {
    // rozdeleni response na header a body
    let parts: Vec<&str> = response.split("\r\n\r\n").collect();

    if parts.len() < 2 {
        // kdyz neni header a body oddeleny
        return HttpResponse::Ok().content_type("text/html").body(response);
    }

    let headers_part = parts[0];
    let body_part = parts[1..].join("\r\n\r\n");

    // zpracovani headeru
    let mut builder = HttpResponse::Ok();
    let mut status_code = 200;
    let mut content_type = "text/html";

    for line in headers_part.lines() {
        if line.is_empty() {
            continue;
        }

        // kontrola status kodu
        if line.starts_with("Status:") {
            if let Some(status_str) = line.split_whitespace().nth(1) {
                if let Ok(code) = status_str.parse::<u16>() {
                    status_code = code;
                }
            }
            continue;
        }

        // rozdeleni headeru na jmeno a hodnotu
        if let Some(colon_pos) = line.find(':') {
            let (name, value) = line.split_at(colon_pos);
            let name = name.trim();
            // odstraneni mezery pred hodnotou
            let value = value[1..].trim();

            if name.eq_ignore_ascii_case("Content-Type") {
                content_type = value;
            } else {
                builder.insert_header((name, value));
            }
        }
    }

    builder.status(StatusCode::from_u16(status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR));
    builder.content_type(content_type);

    builder.body(body_part)
}
