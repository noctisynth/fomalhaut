//! Fomalhaut Wayland session locker entry point.

mod controller_worker;
mod gtk_host;
mod readiness;

use std::ffi::OsStr;

use fomalhaut_pam::{PAM_WORKER_ARGUMENT, run_pam_worker};
use gtk4::glib;

fn main() -> glib::ExitCode {
    if is_exact_worker_invocation() {
        return match run_pam_worker() {
            Ok(()) => glib::ExitCode::SUCCESS,
            Err(_) => glib::ExitCode::FAILURE,
        };
    }
    gtk_host::run()
}

fn is_exact_worker_invocation() -> bool {
    let mut arguments = std::env::args_os();
    let _ = arguments.next();
    arguments.next().as_deref() == Some(OsStr::new(PAM_WORKER_ARGUMENT))
        && arguments.next().is_none()
}
