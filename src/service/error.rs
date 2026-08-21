//! The one place a failure is turned into a status code and a message the
//! client is allowed to see.
//!
//! Every variant keeps its cause as an error source for the logs; `message`
//! decides separately what goes on the wire, so an internal detail cannot leak
//! into a response by being convenient to format.

use crate::fetch::FetchError;
use crate::processing::options::OptionParseError;
use crate::processing::presets::PresetError;
use crate::processing::save::SaveError;
use crate::processing::transform::TransformError;
use crate::processing::ProcessingError;
use crate::url::SourceUrlDecodeError;
use axum::http::StatusCode;
use std::borrow::Cow;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ServiceError {
    #[error(transparent)]
    Fetch(#[from] FetchError),
    #[error("failed to fetch watermark image")]
    WatermarkFetch {
        #[source]
        source: FetchError,
    },
    #[error(transparent)]
    Preset(#[from] PresetError),
    #[error(transparent)]
    OptionParse(#[from] OptionParseError),
    #[error(transparent)]
    SourceUrlDecode(#[from] SourceUrlDecodeError),
    #[error(transparent)]
    Processing(#[from] ProcessingError),
    #[error("failed to decode source image")]
    SourceImageDecode {
        #[source]
        source: libvips::error::Error,
    },
    #[error("{operation} blocking task failed")]
    BlockingTask {
        operation: &'static str,
        #[source]
        source: tokio::task::JoinError,
    },
    #[error("{message}")]
    Response { status: StatusCode, message: String },
}

impl ServiceError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self::Response {
            status,
            message: message.into(),
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            Self::Fetch(_)
            | Self::WatermarkFetch { .. }
            | Self::Preset(_)
            | Self::OptionParse(_)
            | Self::SourceUrlDecode(_)
            | Self::SourceImageDecode { .. } => StatusCode::BAD_REQUEST,
            Self::Processing(ProcessingError::Save(SaveError::Vips { .. } | SaveError::EncoderPanicked { .. })) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::BlockingTask { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Processing(_) => StatusCode::BAD_REQUEST,
            Self::Response { status, .. } => *status,
        }
    }

    pub fn message(&self) -> Cow<'_, str> {
        match self {
            Self::Fetch(FetchError::Request(_)) => Cow::Borrowed("Error fetching image"),
            Self::Fetch(FetchError::ResponseBody(_)) => Cow::Borrowed("Error reading image bytes"),
            Self::Fetch(FetchError::SourceTooLarge { limit, .. }) => Cow::Owned(format!(
                "Source image exceeds the maximum allowed size of {limit} bytes"
            )),
            Self::Fetch(FetchError::SourceNotAllowed) => Cow::Borrowed("Source URL is not allowed"),
            Self::Fetch(FetchError::UpstreamStatus { status }) => {
                Cow::Owned(format!("Source responded with status {status}"))
            }
            Self::WatermarkFetch { .. } => Cow::Borrowed("Failed to fetch watermark image"),
            Self::Preset(error) => Cow::Owned(error.to_string()),
            Self::OptionParse(error) => Cow::Owned(error.to_string()),
            Self::SourceUrlDecode(_) => Cow::Borrowed("Error decoding URL"),
            Self::Processing(ProcessingError::Save(SaveError::UnsupportedFormat { format })) => {
                Cow::Owned(format!("Unsupported output format: {format}"))
            }
            Self::Processing(ProcessingError::Save(_)) => Cow::Borrowed("Failed to encode image"),
            // Every InvalidArgument message describes the caller's own input —
            // an out-of-range zoom, a padded canvas past what libvips will
            // embed — so it is more useful in the response than "error
            // processing image", and carries nothing internal. Vips failures
            // stay generic.
            Self::Processing(ProcessingError::Transform(TransformError::InvalidArgument { message, .. })) => {
                Cow::Borrowed(message.as_str())
            }
            Self::Processing(ProcessingError::ResultTooLarge { width, height, limit }) => Cow::Owned(format!(
                "Processed image would be {width}x{height}, over the {limit}px result dimension limit"
            )),
            Self::Processing(ProcessingError::FrameTooLarge { width, height, limit }) => Cow::Owned(format!(
                "Animation frame is {width}x{height}, over the {limit} pixel frame limit"
            )),
            Self::Processing(_) => Cow::Borrowed("Error processing image"),
            Self::SourceImageDecode { .. } => Cow::Borrowed("Failed to decode source image"),
            Self::BlockingTask { .. } => Cow::Borrowed("Image operation failed"),
            Self::Response { message, .. } => Cow::Borrowed(message),
        }
    }
}
