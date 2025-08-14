use axum::{
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use prometheus::{Encoder, Histogram, IntCounter, IntCounterVec, TextEncoder};
use std::time::Instant;
use tower::{Layer, Service};

lazy_static::lazy_static! {
    static ref HTTP_REQUESTS_TOTAL: IntCounterVec = IntCounterVec::new(
        prometheus::opts!("http_requests_total", "Total number of HTTP requests"),
        &["method", "endpoint", "status"]
    ).unwrap();

    static ref HTTP_REQUEST_DURATION: Histogram = Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "http_request_duration_seconds",
            "HTTP request duration in seconds"
        )
    ).unwrap();

    static ref MEDIA_FILES_SCANNED: IntCounter = IntCounter::new(
        "media_files_scanned_total",
        "Total number of media files scanned"
    ).unwrap();

    static ref MEDIA_FILES_INDEXED: IntCounter = IntCounter::new(
        "media_files_indexed_total",
        "Total number of media files indexed"
    ).unwrap();

    static ref DATABASE_QUERIES: IntCounterVec = IntCounterVec::new(
        prometheus::opts!("database_queries_total", "Total number of database queries"),
        &["operation", "status"]
    ).unwrap();

    static ref SCAN_DURATION: Histogram = Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "scan_duration_seconds",
            "Media scan duration in seconds"
        )
    ).unwrap();
}

pub fn init_metrics() {
    prometheus::register(Box::new(HTTP_REQUESTS_TOTAL.clone())).unwrap();
    prometheus::register(Box::new(HTTP_REQUEST_DURATION.clone())).unwrap();
    prometheus::register(Box::new(MEDIA_FILES_SCANNED.clone())).unwrap();
    prometheus::register(Box::new(MEDIA_FILES_INDEXED.clone())).unwrap();
    prometheus::register(Box::new(DATABASE_QUERIES.clone())).unwrap();
    prometheus::register(Box::new(SCAN_DURATION.clone())).unwrap();
}

#[derive(Clone)]
pub struct MetricsMiddleware;

impl MetricsMiddleware {
    pub fn new() -> Self {
        init_metrics();
        Self
    }
}

impl<S> Layer<S> for MetricsMiddleware {
    type Service = MetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MetricsService { inner }
    }
}

#[derive(Clone)]
pub struct MetricsService<S> {
    inner: S,
}

impl<S> Service<Request> for MetricsService<S>
where
    S: Service<Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let method = req.method().to_string();
        let path = req.uri().path().to_string();
        let start = Instant::now();

        let mut inner = self.inner.clone();

        Box::pin(async move {
            let response = inner.call(req).await?;
            
            let duration = start.elapsed().as_secs_f64();
            let status = response.status().as_u16().to_string();

            HTTP_REQUEST_DURATION.observe(duration);
            HTTP_REQUESTS_TOTAL
                .with_label_values(&[&method, &path, &status])
                .inc();

            Ok(response)
        })
    }
}

pub async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    
    match encoder.encode(&metric_families, &mut buffer) {
        Ok(_) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
            buffer,
        ),
        Err(e) => {
            tracing::error!("Failed to encode metrics: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "text/plain")],
                b"Failed to encode metrics".to_vec(),
            )
        }
    }
}

pub fn record_files_scanned(count: u64) {
    MEDIA_FILES_SCANNED.inc_by(count);
}

pub fn record_files_indexed(count: u64) {
    MEDIA_FILES_INDEXED.inc_by(count);
}

pub fn record_database_query(operation: &str, success: bool) {
    let status = if success { "success" } else { "failure" };
    DATABASE_QUERIES
        .with_label_values(&[operation, status])
        .inc();
}

pub fn record_scan_duration(duration: f64) {
    SCAN_DURATION.observe(duration);
}