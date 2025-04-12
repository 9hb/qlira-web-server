# Qlira Web Server

Vysokovýkonný webový server napsaný v Rustu pomocí frameworku Actix Web s podporou statických souborů, PHP skriptů, JavaScript optimalizací a WebSocketů.

## Popis

Tento projekt implementuje modulární webový server určený pro rychlé a flexibilní servírování webového obsahu. Server je navržen s důrazem na výkon, konfigurovatelnost a snadné nasazení. Díky implementaci v Rustu je zajištěna bezpečnost paměti a vysoký výkon bez nutnosti garbage collectoru.

## Vymoženosti

### Základní funkce

- **HTTP server** - Postavený na frameworku Actix Web pro vysoký výkon a nízkou latenci
- **Konfigurovatelnost** - Kompletní konfigurace pomocí TOML souboru
- **Modulární architektura** - Rozdělení do logických komponent pro snadnou rozšiřitelnost

### Obsluha souborů

- **Efektivní servírování** - Optimalizovaná obsluha statických souborů
- **Podpora různých MIME typů** - Automatická detekce typu obsahu
- **Directory traversal ochrana** - Zabezpečení proti přístupu k souborům mimo povolené adresáře
- **ETag podpora** - Efektivní validace obsahu pomocí ETag hlaviček
- **Podpora Last-Modified** - Kontrola modifikace souborů pro podmíněné požadavky
- **Konfigurovatelný limit velikosti souborů** - Ochrana proti přetížení serverovými soubory

### PHP integrace

- **Duální režim zpracování PHP** - Podpora jak PHP-CGI rozhraní, tak přímé volání PHP.exe
- **Detekce PHP tagů** - Automatická volba mezi režimy podle obsahu souboru
- **Předávání proměnných prostředí** - Správné předávání HTTP hlaviček a proměnných do PHP
- **Timeout mechanismus** - Ochrana proti dlouho běžícím PHP skriptům
- **Zpracování HTTP/POST/GET požadavků** - Plná podpora HTTP metod pro PHP skripty
- **Konfigurovatelná velikost POST dat** - Ochrana proti příliš velkým požadavkům

### JavaScript optimalizace

- **Minifikace JS souborů** - Automatická minifikace pro zmenšení velikosti souborů
- **Volitelný režim vývojáře** - Možnost vypnout minifikaci během vývoje
- **Vlastní cache politika** - Specifické nastavení cachování pro JavaScript soubory

### WebSocket podpora

- **Plná implementace WebSocketů** - Obousměrná komunikace v reálném čase
- **Heartbeat mechanismus** - Automatická kontrola aktivních spojení
- **Timeout konfigurace** - Nastavitelný timeout pro neaktivní spojení
- **Textová a binární data** - Podpora různých typů zpráv
- **Unikátní ID pro spojení** - Sledování a správa jednotlivých WebSocket spojení
- **Konfigurovatelný počet spojení** - Omezení maximálního počtu WebSocket klientů

### Výkonnostní optimalizace

- **Komprese obsahu** - Podpora pro Brotli, Gzip a Deflate kompresi
- **Konfigurovatelný práh komprese** - Nastavení minimální velikosti pro kompresi
- **Inteligentní hlavičky** - Správné nastavení Content-Type, Content-Length a dalších hlaviček
- **Měření výkonu** - Sledování doby zpracování požadavků a jejich statistiky
- **Metriky výkonu** - Implementován sběr metrik pro analýzu výkonu serveru
- **Průměrná doba zpracování** - Výpočet průměrné doby zpracování požadavků

### Cachování

- **Konfigurovatelné HTTP cache** - Nastavitelné strategie cachování
- **Cache-Control hlavičky** - Správné nastavení dle typu obsahu
- **Speciální cache pravidla** - Různé doby cachování pro různé typy souborů:
  - HTML soubory - 1 hodina
  - CSS/JS soubory - 1 týden
  - Fonty (woff2, woff, ttf) - 1 měsíc
  - Obrázky (jpg, png, gif) - 1 rok

### Middleware systém

- **Logovací middleware** - Detailní logování požadavků a odpovědí
- **Error handling middleware** - Zpracování chyb a generování vlastních chybových stránek
- **Výkonnostní měření** - Middleware pro měření doby zpracování požadavků

### Konfigurace serveru

- **Hot-reload konfigurace** - Automatické přenačtení konfigurace při změně souboru
- **HTTP API pro konfiguraci** - REST API pro získání a aktualizaci konfigurace za běhu
- **Konfigurovatelný počet spojení** - Nastavení maximálního počtu současných spojení
- **Možnost bind na specifickou IP adresu** - Flexibilita v síťovém nastavení

### Režim vývojáře

- **Zjednodušený development mode** - Speciální režim optimalizovaný pro vývoj
- **Vypnuté cachování** - V režimu vývojáře je cachování deaktivováno
- **Rozšířené logování** - Detailnější výpisy v režimu vývojáře
- **Přídavné debug informace** - Pomocné informace pro ladění aplikace
- **Přímý přístup k souborům** - Bez minifikace a komprese pro snadnější debugging

### Vlastní chybové stránky

- **Konfigurovatelné chybové stránky** - Možnost nastavit vlastní stránky pro různé HTTP chybové kódy
- **Standardní obsluha chyb** - Výchozí šablony pro nejběžnější HTTP chyby (404, 500, atd.)
- **Komplexní pokrytí stavových kódů** - Podpora pro všechny standardní HTTP chybové kódy

### Bezpečnost

- **Ochrana proti directory traversal** - Zabezpečení proti únikům souborů mimo povolené adresáře
- **Limity velikosti požadavků** - Ochrana proti DoS útokům
- **Konfigurovatelné timeouty** - Ochrana proti pomalým klientům
- **Validace vstupů** - Důsledná kontrola všech uživatelských vstupů

## Instalace

```bash
# Klonování repozitáře
git clone https://github.com/github/qlira-web-server.git
cd rust-web-server

# Kompilace projektu
cargo build --release

# Spuštění serveru
cargo run --release
```

## Konfigurace

Server je možné nakonfigurovat pomocí souboru `config/server.toml`. Hlavní konfigurovatelné parametry:

```toml
# Příklad konfigurace
server_directory = "server"
port = 8080
timeout = 30
max_connections = 1000
bind_address = "0.0.0.0"

# Nastavení cachování
cache_enabled = true
default_max_age = 86400  # 1 den

# Nastavení PHP
php_enabled = true
php_cgi_path = "php/php-cgi.exe"
php_exe_path = "php/php.exe"
php_root_dir = "web"
php_timeout = 30

# Websockety
enable_websockets = true
websocket_path = "/ws"
websocket_max_connections = 1000
websocket_timeout = 60

# Další nastavení...
```

## Použití API

Server poskytuje REST API pro správu konfigurace:

- `GET /api/config` - Získání aktuální konfigurace
- `POST /api/config` - Aktualizace konfigurace
- `POST /api/config/reload` - Ruční přenačtení konfigurace

## Licence

Tento projekt je licensován pod [MIT licencí](LICENSE).
