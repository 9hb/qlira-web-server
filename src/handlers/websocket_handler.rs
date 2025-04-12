use actix::{ AsyncContext, Actor, StreamHandler, ActorContext }; // Added ActorContext trait
use actix_web::{ web, Error, HttpRequest, HttpResponse };
use actix_web_actors::ws;
use std::time::{ Duration, Instant };
use std::sync::Arc;
use crate::config::ConfigManager;
use rand::Rng;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

struct WebSocketSession {
    id: usize,
    /// cas posledniho pingu
    heartbeat: Instant,
    /// konfigurace pro WebSocket
    config: web::Data<Arc<ConfigManager>>,
}

impl Actor for WebSocketSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.heartbeat(ctx);

        println!("WebSocket spojeni {} bylo zahajeno", self.id);
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        println!("WebSocket spojeni {} bylo ukonceno", self.id);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.heartbeat = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                // zpracovani textovych zprav
                println!("WebSocket client {} poslal zpravu: {}", self.id, text);
                let response = format!("obdrzel jsem zpravu: {}", text);
                ctx.text(response);
            }
            Ok(ws::Message::Binary(bin)) => {
                // zpracovani binarnich zprav
                println!(
                    "WebSocket client {}: poslal binarni data o velikosti {}",
                    self.id,
                    bin.len()
                );
                ctx.binary(bin);
            }
            Ok(ws::Message::Close(reason)) => {
                // client se odpojil
                println!("WebSocket client {} byl odpojen", self.id);
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

impl WebSocketSession {
    // pomocna metoda pro odeslani ping zpravy
    fn heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        let config = self.config.get_config();
        let timeout_duration = Duration::from_secs(config.websocket_timeout);

        ctx.run_interval(HEARTBEAT_INTERVAL, move |act, ctx| {
            // zkontrolujeme, jestli klient odpovida
            if Instant::now().duration_since(act.heartbeat) > timeout_duration {
                println!("WebSocket klient {} prekrocil timeout, odpojuji", act.id);
                ctx.stop();
                return;
            }

            ctx.ping(b"");
        });
    }
}

pub async fn websocket_handler(
    req: HttpRequest,
    stream: web::Payload,
    config_manager: web::Data<Arc<ConfigManager>>
) -> Result<HttpResponse, Error> {
    // zkontrolovat, jestli jsou WebSockety povoleny
    let config = config_manager.get_config();
    if !config.enable_websockets {
        return Ok(HttpResponse::NotFound().body("WebSockets nejsou povoleny"));
    }

    // generovat unikatni ID pro session using thread_rng instead of random
    let session_id = rand::thread_rng().gen::<usize>();

    println!("nove websocket spojeni: {}", session_id);

    ws::start(
        WebSocketSession {
            id: session_id,
            heartbeat: Instant::now(),
            config: config_manager,
        },
        &req,
        stream
    )
}
