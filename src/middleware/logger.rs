use actix_web::{ dev::ServiceRequest, dev::ServiceResponse, Error };
use std::time::Instant;
use std::future::Future;
use std::pin::Pin;
use std::task::{ Context, Poll };
use actix_web::dev::{ Service, Transform };
use futures::future::{ ok, Ready };

// funkce middlwaru pro ServiceRequest/ServiceResponse
pub async fn log_requests<B>(
    req: ServiceRequest,
    srv: impl Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>
) -> Result<ServiceResponse<B>, Error> {
    let start_time = Instant::now();

    println!("prijmul jsem request: {} {}", req.method(), req.uri());

    let res = srv.call(req).await?;

    let duration = start_time.elapsed();
    println!("request zpracovan za {:?}", duration);

    Ok(res)
}

// middleware struktura pro logovani
pub struct Logger;

impl Logger {
    pub fn new() -> Self {
        Logger
    }
}

impl<S, B> Transform<S, ServiceRequest>
    for Logger
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
        B: 'static
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = LoggerMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(LoggerMiddleware { service })
    }
}

pub struct LoggerMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest>
    for LoggerMiddleware<S>
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
        B: 'static
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let start_time = Instant::now();

        println!("prijmul jsem request: {} {}", req.method(), req.uri());

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;

            let duration = start_time.elapsed();
            println!("request zpracovan za {:?}", duration);

            Ok(res)
        })
    }
}
