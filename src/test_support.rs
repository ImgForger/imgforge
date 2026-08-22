//! libvips initialisation shared by every test module.
//!
//! libvips is a process-global library: it is initialised once and shut down
//! once. Two modules each holding their own `VipsApp` would initialise it
//! twice and, worse, shut it down while the other still holds images. Any test
//! that touches a vips operation — including the format probe behind
//! `is_format_supported` — has to go through this.

use lazy_static::lazy_static;
use libvips::VipsApp;

lazy_static! {
    static ref APP: VipsApp = {
        let app = VipsApp::new("imgforge-tests", false).expect("Cannot initialize libvips");
        app.concurrency_set(1);
        app
    };
}

pub fn init_vips() {
    let _ = &*APP;
}

/// libvips reports the useful part of a failure in a global buffer that the
/// crate's error type does not carry, so tests that need to tell one failure
/// from another have to read it directly. It is sticky — clear it first.
pub fn clear_vips_error() {
    APP.error_clear();
}

pub fn vips_error_buffer() -> String {
    APP.error_buffer().unwrap_or_default().to_string()
}
