//! `ext-session-lock-v1` ownership and one WebView per compositor output.

use std::{
    cell::{Cell, RefCell},
    error::Error,
    fmt,
    path::PathBuf,
    rc::Rc,
    sync::mpsc::{Receiver, TryRecvError},
    time::Duration,
};

use fomalhaut_config::AppConfig;
use fomalhaut_gtk::{
    ApplicationHandle, HostedView, ViewCallbacks, build_web_view, run_application,
};
use fomalhaut_pam::CurrentUserIdentity;
use fomalhaut_web::{protocol::RuntimeMode, theme::ThemeSource};
use gtk4 as gtk;
use gtk4::{gdk, glib, prelude::*};
use gtk4_session_lock::{Instance, is_supported};

use crate::controller_worker::{LockerViewAction, NativeEvent, ViewController, WorkerHandle};
use crate::readiness::notify_ready;

const APPLICATION_ID: &str = "org.fomalhautdm.FomalhautLock";
const NATIVE_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Runs the locker until an authorized compositor release or a pre-lock failure.
pub fn run() -> glib::ExitCode {
    run_application(APPLICATION_ID, activate)
}

fn activate(application: ApplicationHandle) -> Result<(), HostError> {
    if !is_supported() {
        return Err(HostError::SessionLockUnsupported);
    }
    let config = AppConfig::load().map_err(|_| HostError::Configuration)?;
    let locker = config.for_locker();
    let theme_directory = locker.theme_directory().map(PathBuf::from);
    validate_theme(theme_directory.as_ref())?;
    let identity = CurrentUserIdentity::discover().map_err(|_| HostError::Identity)?;
    let (worker, native) = WorkerHandle::spawn(identity).map_err(|_| HostError::WorkerSpawn)?;
    let instance = Instance::new();
    let state = Rc::new(LockHost {
        application,
        instance,
        worker,
        theme_directory,
        zoom_level: locker.display().scale(),
        lock_requested: Cell::new(false),
        lock_acquired: Cell::new(false),
        lock_failure_sent: Cell::new(false),
        next_surface: Cell::new(1),
        surfaces: RefCell::new(Vec::new()),
    });
    connect_session_lock(&state);
    poll_native_events(&state, native);
    Ok(())
}

fn validate_theme(theme_directory: Option<&PathBuf>) -> Result<(), HostError> {
    if let Some(directory) = theme_directory {
        ThemeSource::external(directory.clone()).map_err(|_| HostError::ExternalTheme)?;
    }
    Ok(())
}

struct LockHost {
    application: ApplicationHandle,
    instance: Instance,
    worker: Rc<WorkerHandle>,
    theme_directory: Option<PathBuf>,
    zoom_level: f64,
    lock_requested: Cell<bool>,
    lock_acquired: Cell<bool>,
    lock_failure_sent: Cell<bool>,
    next_surface: Cell<u64>,
    surfaces: RefCell<Vec<Rc<MonitorSurface>>>,
}

impl LockHost {
    fn request_lock(&self) {
        if self.lock_requested.replace(true) {
            return;
        }
        if !self.instance.lock() {
            self.report_lock_failure();
        }
    }

    fn report_lock_failure(&self) {
        if self.lock_failure_sent.replace(true) {
            return;
        }
        if self.worker.mark_lock_failed().is_err() {
            self.application.quit_failure();
        }
    }

    fn add_monitor(self: &Rc<Self>, monitor: &gdk::Monitor) -> Result<(), HostError> {
        let identifier = self.next_surface.get();
        let next = identifier
            .checked_add(1)
            .ok_or(HostError::SurfaceIdentifierExhausted)?;
        let surface = MonitorSurface::new(
            identifier,
            &self.application,
            Rc::clone(&self.worker),
            self.theme_directory.clone(),
            self.zoom_level,
        )?;
        self.next_surface.set(next);
        self.instance
            .assign_window_to_monitor(surface.window(), monitor);
        let weak_host = Rc::downgrade(self);
        surface.window().connect_destroy(move |_| {
            if let Some(host) = weak_host.upgrade() {
                host.remove_surface(identifier);
            }
        });
        surface.window().present();
        self.surfaces.borrow_mut().push(surface);
        Ok(())
    }

    fn remove_surface(&self, identifier: u64) {
        self.surfaces
            .borrow_mut()
            .retain(|surface| surface.identifier() != identifier);
    }

    fn show_trusted_failure(&self, message: &'static str) {
        for surface in self.surfaces.borrow().iter() {
            surface.show_failure(message);
        }
    }

    fn handle_native(&self, event: NativeEvent) -> glib::ControlFlow {
        match event {
            NativeEvent::BackendReady => self.request_lock(),
            NativeEvent::LockAcquired => {
                self.lock_acquired.set(true);
                if let Err(error) = notify_ready() {
                    eprintln!("Fomalhaut lock readiness notification failed: {error}");
                }
                eprintln!("Fomalhaut session lock acquired and ready");
            }
            NativeEvent::LockFailed => {
                eprintln!("Fomalhaut could not acquire the compositor session lock");
                self.application.quit_failure();
                return glib::ControlFlow::Break;
            }
            NativeEvent::Unlock => {
                if !self.lock_acquired.get() || !self.instance.is_locked() {
                    self.show_trusted_failure("Unlock authorization arrived in an invalid state.");
                    if !self.instance.is_locked() {
                        self.application.quit_failure();
                        return glib::ControlFlow::Break;
                    }
                } else {
                    self.instance.unlock();
                }
            }
            NativeEvent::Released => {
                if let Some(display) = gdk::Display::default() {
                    display.sync();
                }
                eprintln!("Fomalhaut session lock released after authorization");
                self.worker.shutdown();
                self.application.quit();
                return glib::ControlFlow::Break;
            }
            NativeEvent::Fatal(message) => {
                eprintln!("Fomalhaut locker entered trusted failure mode: {message}");
                self.show_trusted_failure(
                    "Authentication is unavailable. The session remains locked.",
                );
                if !self.instance.is_locked() {
                    self.application.quit_failure();
                    return glib::ControlFlow::Break;
                }
            }
        }
        glib::ControlFlow::Continue
    }
}

fn connect_session_lock(state: &Rc<LockHost>) {
    let monitor_state = Rc::downgrade(state);
    state.instance.connect_monitor(move |_, monitor| {
        let Some(state) = monitor_state.upgrade() else {
            return;
        };
        if let Err(error) = state.add_monitor(monitor) {
            eprintln!("Fomalhaut could not create a monitor lock surface: {error}");
            if state.instance.is_locked() {
                state.show_trusted_failure(
                    "A display could not be secured. The session remains locked.",
                );
            } else {
                state.report_lock_failure();
            }
        }
    });

    let locked_state = Rc::downgrade(state);
    state.instance.connect_locked(move |_| {
        if let Some(state) = locked_state.upgrade()
            && state.worker.mark_lock_acquired().is_err()
        {
            state.show_trusted_failure(
                "The lock controller is unavailable. The session remains locked.",
            );
        }
    });

    let failed_state = Rc::downgrade(state);
    state.instance.connect_failed(move |_| {
        if let Some(state) = failed_state.upgrade() {
            state.report_lock_failure();
        }
    });

    let released_state = Rc::downgrade(state);
    state.instance.connect_unlocked(move |_| {
        if let Some(state) = released_state.upgrade()
            && state.worker.mark_lock_released().is_err()
        {
            state.application.quit_failure();
        }
    });
}

fn poll_native_events(state: &Rc<LockHost>, events: Receiver<NativeEvent>) {
    let state = Rc::clone(state);
    glib::timeout_add_local(NATIVE_POLL_INTERVAL, move || {
        loop {
            match events.try_recv() {
                Ok(event) => {
                    if state.handle_native(event).is_break() {
                        return glib::ControlFlow::Break;
                    }
                }
                Err(TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => {
                    state.show_trusted_failure(
                        "Authentication is unavailable. The session remains locked.",
                    );
                    if !state.instance.is_locked() {
                        state.application.quit_failure();
                    }
                    return glib::ControlFlow::Break;
                }
            }
        }
    });
}

struct MonitorSurface {
    identifier: u64,
    window: gtk::ApplicationWindow,
    stack: gtk::Stack,
    fallback_label: gtk::Label,
    worker: Rc<WorkerHandle>,
    theme_directory: Option<PathBuf>,
    zoom_level: f64,
    view: RefCell<Option<HostedView<ViewController>>>,
}

impl MonitorSurface {
    fn new(
        identifier: u64,
        application: &ApplicationHandle,
        worker: Rc<WorkerHandle>,
        theme_directory: Option<PathBuf>,
        zoom_level: f64,
    ) -> Result<Rc<Self>, HostError> {
        let fallback_label = gtk::Label::builder()
            .label("Securing the authentication interface…")
            .wrap(true)
            .build();
        let retry = gtk::Button::with_label("Retry interface");
        let fallback = gtk::Box::new(gtk::Orientation::Vertical, 16);
        fallback.set_halign(gtk::Align::Center);
        fallback.set_valign(gtk::Align::Center);
        fallback.append(&fallback_label);
        fallback.append(&retry);
        let stack = gtk::Stack::new();
        stack.add_named(&fallback, Some("fallback"));
        stack.set_visible_child_name("fallback");
        let window = gtk::ApplicationWindow::builder()
            .application(application.application())
            .title("Fomalhaut Lock")
            .child(&stack)
            .build();
        let surface = Rc::new(Self {
            identifier,
            window,
            stack,
            fallback_label,
            worker,
            theme_directory,
            zoom_level,
            view: RefCell::new(None),
        });
        let retry_surface = Rc::downgrade(&surface);
        retry.connect_clicked(move |_| {
            if let Some(surface) = retry_surface.upgrade()
                && let Err(error) = surface.rebuild_view()
            {
                eprintln!("Fomalhaut could not rebuild a monitor WebView: {error}");
                surface.show_failure("The authentication interface could not be restarted.");
            }
        });
        surface.rebuild_view()?;
        Ok(surface)
    }

    const fn identifier(&self) -> u64 {
        self.identifier
    }

    const fn window(&self) -> &gtk::ApplicationWindow {
        &self.window
    }

    fn rebuild_view(self: &Rc<Self>) -> Result<(), HostError> {
        if let Some(previous) = self.view.borrow_mut().take() {
            previous.shutdown();
            self.stack.remove(previous.web_view());
        }
        self.stack.set_visible_child_name("fallback");
        self.fallback_label
            .set_label("Securing the authentication interface…");
        let (controller, outputs) = self
            .worker
            .register_view()
            .map_err(|_| HostError::ViewRegistration)?;
        let ready_surface = Rc::downgrade(self);
        let failure_surface = Rc::downgrade(self);
        let callbacks = ViewCallbacks::new(
            move || {
                if let Some(surface) = ready_surface.upgrade() {
                    surface.stack.set_visible_child_name("theme");
                }
            },
            |action: LockerViewAction| match action {},
            move |failure| {
                eprintln!("Fomalhaut monitor WebView entered fallback mode: {failure}");
                if let Some(surface) = failure_surface.upgrade() {
                    surface.show_failure(
                        "The authentication interface failed. The session remains locked.",
                    );
                }
            },
        );
        let theme = match &self.theme_directory {
            Some(directory) => {
                ThemeSource::external(directory.clone()).map_err(|_| HostError::ExternalTheme)?
            }
            None => ThemeSource::Embedded,
        };
        let view = build_web_view(
            theme,
            RuntimeMode::Locker,
            self.zoom_level,
            controller,
            outputs,
            callbacks,
        )
        .map_err(|_| HostError::SharedView)?;
        self.stack.add_named(view.web_view(), Some("theme"));
        view.load_theme();
        self.view.replace(Some(view));
        Ok(())
    }

    fn show_failure(&self, message: &'static str) {
        self.fallback_label.set_label(message);
        self.stack.set_visible_child_name("fallback");
    }
}

impl Drop for MonitorSurface {
    fn drop(&mut self) {
        if let Some(view) = self.view.get_mut().take() {
            view.shutdown();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostError {
    Configuration,
    ExternalTheme,
    Identity,
    WorkerSpawn,
    SessionLockUnsupported,
    SurfaceIdentifierExhausted,
    ViewRegistration,
    SharedView,
}

impl fmt::Display for HostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "the system configuration is invalid",
            Self::ExternalTheme => "the configured locker theme is invalid",
            Self::Identity => "the current session account could not be resolved",
            Self::WorkerSpawn => "the locker authentication worker could not be started",
            Self::SessionLockUnsupported => "the compositor does not support ext-session-lock-v1",
            Self::SurfaceIdentifierExhausted => "the monitor surface identifier was exhausted",
            Self::ViewRegistration => "the monitor view could not register with the controller",
            Self::SharedView => "the shared WebKitGTK view could not be initialized",
        })
    }
}

impl Error for HostError {}
