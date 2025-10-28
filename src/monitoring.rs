use metrics::{describe_counter, describe_gauge, describe_histogram, Unit};
use std::sync::Once;

static REGISTER: Once = Once::new();

pub fn register_metrics() {
    REGISTER.call_once(|| {
        describe_histogram!(
            "image_processing_duration_seconds",
            Unit::Seconds,
            "Image processing duration in seconds"
        );
        describe_histogram!(
            "source_image_fetch_duration_seconds",
            Unit::Seconds,
            "Source image fetch duration in seconds"
        );
        describe_counter!("processed_images_total", "Total number of processed images");
        describe_counter!("source_images_fetched_total", "Total number of source images fetched");
        describe_counter!("cache_hits_total", "Total number of cache hits");
        describe_counter!("cache_misses_total", "Total number of cache misses");
        describe_counter!("status_codes_total", "Total number of response status codes");
        describe_gauge!(
            "vips_tracked_mem_bytes",
            Unit::Bytes,
            "Current libvips tracked memory usage in bytes"
        );
        describe_gauge!(
            "vips_tracked_mem_highwater_bytes",
            Unit::Bytes,
            "Peak libvips tracked memory usage in bytes"
        );
        describe_gauge!("vips_tracked_allocs", "Number of active libvips tracked allocations");
    });
}

pub fn observe_image_processing_duration(format: &str, duration_seconds: f64) {
    let format_label = format.to_owned();
    metrics::histogram!("image_processing_duration_seconds", "format" => format_label).record(duration_seconds);
}

pub fn increment_processed_images(format: &str) {
    let format_label = format.to_owned();
    metrics::counter!("processed_images_total", "format" => format_label).increment(1);
}

pub fn observe_source_image_fetch_duration(duration_seconds: f64) {
    metrics::histogram!("source_image_fetch_duration_seconds").record(duration_seconds);
}

pub fn increment_source_images_fetched(status: &str) {
    let status_label = status.to_owned();
    metrics::counter!("source_images_fetched_total", "status" => status_label).increment(1);
}

pub fn increment_cache_hit(cache_type: &str) {
    let cache_type_label = cache_type.to_owned();
    metrics::counter!("cache_hits_total", "cache_type" => cache_type_label).increment(1);
}

pub fn increment_cache_miss(cache_type: &str) {
    let cache_type_label = cache_type.to_owned();
    metrics::counter!("cache_misses_total", "cache_type" => cache_type_label).increment(1);
}

pub fn increment_status_code(status: &str) {
    let status_label = status.to_owned();
    metrics::counter!("status_codes_total", "status" => status_label).increment(1);
}

pub fn update_vips_metrics(vips_app: &std::sync::Arc<libvips::VipsApp>) {
    metrics::gauge!("vips_tracked_mem_bytes").set(vips_app.tracked_get_mem() as f64);
    metrics::gauge!("vips_tracked_mem_highwater_bytes").set(vips_app.tracked_get_mem_highwater() as f64);
    metrics::gauge!("vips_tracked_allocs").set(vips_app.tracked_get_allocs() as f64);
}
