use lazy_static::lazy_static;
use prometheus::{HistogramVec, IntCounterVec, IntGauge, Opts, Registry};

lazy_static! {
    pub static ref HTTP_REQUESTS_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        prometheus::HistogramOpts::new("http_requests_duration_seconds", "HTTP request duration in seconds"),
        &["method", "path"]
    )
    .unwrap();
    pub static ref IMAGE_PROCESSING_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        prometheus::HistogramOpts::new(
            "image_processing_duration_seconds",
            "Image processing duration in seconds"
        ),
        &["format"]
    )
    .unwrap();
    pub static ref SOURCE_IMAGE_FETCH_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        prometheus::HistogramOpts::new(
            "source_image_fetch_duration_seconds",
            "Source image fetch duration in seconds"
        ),
        &[]
    )
    .unwrap();
    pub static ref PROCESSED_IMAGES_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("processed_images_total", "Total number of processed images"),
        &["format"]
    )
    .unwrap();
    pub static ref SOURCE_IMAGES_FETCHED_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("source_images_fetched_total", "Total number of source images fetched"),
        &["status"]
    )
    .unwrap();
    pub static ref CACHE_HITS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("cache_hits_total", "Total number of cache hits"),
        &["cache_type"]
    )
    .unwrap();
    pub static ref CACHE_MISSES_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("cache_misses_total", "Total number of cache misses"),
        &["cache_type"]
    )
    .unwrap();
    pub static ref STATUS_CODES_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("status_codes_total", "Total number of response status codes"),
        &["status"]
    )
    .unwrap();
    pub static ref VIPS_TRACKED_MEM_BYTES: IntGauge = IntGauge::new(
        "vips_tracked_mem_bytes",
        "Current libvips tracked memory usage in bytes"
    )
    .unwrap();
    pub static ref VIPS_TRACKED_MEM_HIGHWATER_BYTES: IntGauge = IntGauge::new(
        "vips_tracked_mem_highwater_bytes",
        "Peak libvips tracked memory usage in bytes"
    )
    .unwrap();
    pub static ref VIPS_TRACKED_ALLOCS: IntGauge =
        IntGauge::new("vips_tracked_allocs", "Number of active libvips tracked allocations").unwrap();
}

pub fn register_metrics(registry: &Registry) {
    registry
        .register(Box::new(HTTP_REQUESTS_DURATION_SECONDS.clone()))
        .unwrap();
    registry
        .register(Box::new(IMAGE_PROCESSING_DURATION_SECONDS.clone()))
        .unwrap();
    registry
        .register(Box::new(SOURCE_IMAGE_FETCH_DURATION_SECONDS.clone()))
        .unwrap();
    registry.register(Box::new(PROCESSED_IMAGES_TOTAL.clone())).unwrap();
    registry
        .register(Box::new(SOURCE_IMAGES_FETCHED_TOTAL.clone()))
        .unwrap();
    registry.register(Box::new(CACHE_HITS_TOTAL.clone())).unwrap();
    registry.register(Box::new(CACHE_MISSES_TOTAL.clone())).unwrap();
    registry.register(Box::new(STATUS_CODES_TOTAL.clone())).unwrap();
    registry.register(Box::new(VIPS_TRACKED_MEM_BYTES.clone())).unwrap();
    registry
        .register(Box::new(VIPS_TRACKED_MEM_HIGHWATER_BYTES.clone()))
        .unwrap();
    registry.register(Box::new(VIPS_TRACKED_ALLOCS.clone())).unwrap();
}

pub fn update_vips_metrics(vips_app: &std::sync::Arc<libvips::VipsApp>) {
    VIPS_TRACKED_MEM_BYTES.set(vips_app.tracked_get_mem() as i64);
    VIPS_TRACKED_MEM_HIGHWATER_BYTES.set(vips_app.tracked_get_mem_highwater() as i64);
    VIPS_TRACKED_ALLOCS.set(vips_app.tracked_get_allocs() as i64);
}
