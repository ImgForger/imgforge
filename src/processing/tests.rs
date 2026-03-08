#[cfg(test)]
#[path = "tests_support.rs"]
mod tests_support;

#[cfg(test)]
#[path = "tests/options_parse_tests.rs"]
mod options_parse_tests;

#[cfg(test)]
#[path = "tests/resize_tests.rs"]
mod resize_tests;

#[cfg(test)]
#[path = "tests/padding_extend_tests.rs"]
mod padding_extend_tests;

#[cfg(test)]
#[path = "tests/exif_tests.rs"]
mod exif_tests;

#[cfg(test)]
#[path = "tests/effects_tests.rs"]
mod effects_tests;

#[cfg(test)]
#[path = "tests/watermark_tests.rs"]
mod watermark_tests;

#[cfg(test)]
#[path = "tests/pipeline_tests.rs"]
mod pipeline_tests;
