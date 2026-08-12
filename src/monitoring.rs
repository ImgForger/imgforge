use metrics::{describe_counter, describe_gauge, describe_histogram, Unit};
use std::sync::Once;
use std::time::Instant;

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
        describe_histogram!(
            "image_operation_semaphore_wait_duration_seconds",
            Unit::Seconds,
            "Time spent waiting for an imgforge image-operation permit"
        );
        describe_histogram!(
            "image_operation_blocking_queue_duration_seconds",
            Unit::Seconds,
            "Time spent waiting for a Tokio blocking thread after submission"
        );
        describe_histogram!(
            "image_operation_execution_duration_seconds",
            Unit::Seconds,
            "Time spent executing an image operation on a blocking thread"
        );
        describe_gauge!(
            "image_operation_concurrency_limit",
            "Configured maximum number of concurrent image operations"
        );
        describe_gauge!(
            "image_operations_active",
            "Image operations currently executing on blocking threads"
        );
        describe_gauge!(
            "image_operations_waiting",
            "Image operations waiting for a permit or blocking thread"
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImageOperation {
    Process,
    Info,
    Watermark,
}

impl ImageOperation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::Info => "info",
            Self::Watermark => "watermark",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImageOperationPhase {
    SemaphoreWait,
    BlockingQueue,
    Execution,
}

/// Records elapsed time when dropped, including on early returns and unwinding.
#[must_use = "the timer must be retained until the measured phase ends"]
pub(crate) struct ImageOperationTimer {
    operation: ImageOperation,
    phase: ImageOperationPhase,
    started_at: Instant,
}

impl ImageOperationTimer {
    pub(crate) fn start(operation: ImageOperation, phase: ImageOperationPhase) -> Self {
        Self {
            operation,
            phase,
            started_at: Instant::now(),
        }
    }
}

impl Drop for ImageOperationTimer {
    fn drop(&mut self) {
        let elapsed = self.started_at.elapsed().as_secs_f64();
        let operation = self.operation.as_str();
        match self.phase {
            ImageOperationPhase::SemaphoreWait => {
                metrics::histogram!("image_operation_semaphore_wait_duration_seconds", "operation" => operation)
                    .record(elapsed);
            }
            ImageOperationPhase::BlockingQueue => {
                metrics::histogram!("image_operation_blocking_queue_duration_seconds", "operation" => operation)
                    .record(elapsed);
            }
            ImageOperationPhase::Execution => {
                metrics::histogram!("image_operation_execution_duration_seconds", "operation" => operation)
                    .record(elapsed);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImageOperationActivity {
    Waiting,
    Active,
}

/// Keeps an image-operation activity gauge balanced across errors and unwinding.
#[must_use = "the guard must be retained while the operation is in this state"]
pub(crate) struct ImageOperationActivityGuard {
    operation: ImageOperation,
    activity: ImageOperationActivity,
}

impl ImageOperationActivityGuard {
    pub(crate) fn waiting(operation: ImageOperation) -> Self {
        metrics::gauge!("image_operations_waiting", "operation" => operation.as_str()).increment(1.0);
        Self {
            operation,
            activity: ImageOperationActivity::Waiting,
        }
    }

    pub(crate) fn active(operation: ImageOperation) -> Self {
        metrics::gauge!("image_operations_active", "operation" => operation.as_str()).increment(1.0);
        Self {
            operation,
            activity: ImageOperationActivity::Active,
        }
    }
}

impl Drop for ImageOperationActivityGuard {
    fn drop(&mut self) {
        let operation = self.operation.as_str();
        match self.activity {
            ImageOperationActivity::Waiting => {
                metrics::gauge!("image_operations_waiting", "operation" => operation).decrement(1.0);
            }
            ImageOperationActivity::Active => {
                metrics::gauge!("image_operations_active", "operation" => operation).decrement(1.0);
            }
        }
    }
}

pub(crate) fn set_image_operation_concurrency_limit(limit: usize) {
    metrics::gauge!("image_operation_concurrency_limit").set(limit as f64);
    for operation in [ImageOperation::Process, ImageOperation::Info, ImageOperation::Watermark] {
        metrics::gauge!("image_operations_active", "operation" => operation.as_str()).set(0.0);
        metrics::gauge!("image_operations_waiting", "operation" => operation.as_str()).set(0.0);
    }
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
