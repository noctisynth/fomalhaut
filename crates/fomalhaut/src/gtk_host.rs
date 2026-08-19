//! Greeter-specific GTK application and ordinary fullscreen window composition.

use std::{env, error::Error, fmt, path::PathBuf, rc::Rc};

use fomalhaut_config::{AppConfig, ThemeSelector, UiLocale as ConfigUiLocale};
use fomalhaut_gtk::{ApplicationHandle, ViewCallbacks, build_web_view, run_application};
use fomalhaut_session::{DiscoveryConfig, SessionKind as CatalogSessionKind, discover};
use fomalhaut_web::{
    controller::TrustedSession,
    protocol::{
        MAX_SESSIONS, RuntimeMode, SessionKind as WebSessionKind, SessionSummary, UiLocale,
    },
    theme::{ThemeSource, discover_theme},
};
use gtk4 as gtk;
use gtk4::{glib, prelude::*};

use crate::controller_worker::{GreeterAction, WorkerHandle};

const APPLICATION_ID: &str = "org.fomalhautdm.Fomalhaut";

/// Runs the greeter until its GTK application exits.
pub fn run() -> glib::ExitCode {
    run_application(APPLICATION_ID, activate)
}

fn activate(application: ApplicationHandle) -> Result<(), HostError> {
    let window = build_window(&application)?;
    window.fullscreen();
    window.present();
    Ok(())
}

fn build_window(application: &ApplicationHandle) -> Result<gtk::ApplicationWindow, HostError> {
    let config = AppConfig::load().map_err(|_| HostError::Configuration)?;
    let (theme, discovery, user_discovery, power, display, locale) =
        config.for_greeter().into_parts();
    let theme_directory = resolve_theme(theme)?;
    let theme = match theme_directory {
        Some(directory) => {
            ThemeSource::external(directory).map_err(|_| HostError::ExternalTheme)?
        }
        None => ThemeSource::Embedded,
    };
    let socket_path = greetd_socket_path()?;
    let sessions = discover_trusted_sessions(&discovery)?;
    let (worker, outputs) = WorkerHandle::spawn(
        socket_path,
        sessions,
        user_discovery,
        power,
        protocol_locale(locale),
    )
    .map_err(|_| HostError::WorkerSpawn)?;
    let worker = Rc::new(worker);

    let terminal_application = application.clone();
    let failure_application = application.clone();
    let callbacks = ViewCallbacks::new(
        || eprintln!("Fomalhaut authentication controller connected to greetd"),
        move |action| match action {
            GreeterAction::SessionStarted => {
                eprintln!("Fomalhaut trusted user session started; host is exiting");
                terminal_application.quit();
            }
        },
        move |failure| {
            eprintln!("Fomalhaut shared WebView host failed: {failure}");
            failure_application.quit_failure();
        },
    );
    let view = build_web_view(
        theme,
        RuntimeMode::Greeter,
        display.scale(),
        worker,
        outputs,
        callbacks,
    )
    .map_err(|_| HostError::SharedView)?;

    let window = gtk::ApplicationWindow::builder()
        .application(application.application())
        .title("Fomalhaut")
        .default_width(1280)
        .default_height(720)
        .child(view.web_view())
        .build();
    let closing_view = view.clone();
    window.connect_close_request(move |_| {
        eprintln!("Fomalhaut host window is closing");
        closing_view.shutdown();
        glib::Propagation::Proceed
    });

    view.load_theme();
    Ok(window)
}

fn resolve_theme(theme: Option<ThemeSelector>) -> Result<Option<PathBuf>, HostError> {
    match theme {
        Some(ThemeSelector::Directory(directory)) => Ok(Some(directory)),
        Some(ThemeSelector::Id(id)) => {
            let discovered = discover_theme(&id).map_err(|_| HostError::ExternalTheme)?;
            if !discovered.conflicts().is_empty() {
                eprintln!(
                    "Fomalhaut theme ID {id} matched multiple directories; selected {}",
                    discovered.directory().display()
                );
                for conflict in discovered.conflicts() {
                    eprintln!(
                        "Fomalhaut ignored lower-priority theme {}",
                        conflict.display()
                    );
                }
            }
            Ok(Some(discovered.into_directory()))
        }
        None => Ok(None),
    }
}

const fn protocol_locale(locale: ConfigUiLocale) -> UiLocale {
    match locale {
        ConfigUiLocale::En => UiLocale::En,
        ConfigUiLocale::ZhCn => UiLocale::ZhCn,
    }
}

fn discover_trusted_sessions(config: &DiscoveryConfig) -> Result<Vec<TrustedSession>, HostError> {
    let report = discover(config).map_err(|_| HostError::SessionDiscovery)?;
    if report.catalog().is_empty() {
        return Err(HostError::NoSessions);
    }
    if report.catalog().len() > MAX_SESSIONS {
        return Err(HostError::InvalidSessionCatalog);
    }

    report
        .catalog()
        .sessions()
        .map(|session| {
            let kind = match session.kind() {
                CatalogSessionKind::Wayland => WebSessionKind::Wayland,
                CatalogSessionKind::X11 => WebSessionKind::X11,
            };
            let summary = SessionSummary::new(
                session.id().as_str().to_owned(),
                session.name().to_owned(),
                kind,
            )
            .map_err(|_| HostError::InvalidSessionCatalog)?;
            let command = report
                .catalog()
                .command(session.id())
                .map_err(|_| HostError::InvalidSessionCatalog)?;
            Ok(TrustedSession::new(summary, command))
        })
        .collect()
}

fn greetd_socket_path() -> Result<PathBuf, HostError> {
    let socket = env::var_os("GREETD_SOCK").ok_or(HostError::MissingGreetdSocket)?;
    if socket.as_os_str().is_empty() {
        return Err(HostError::InvalidGreetdSocket);
    }
    let path = PathBuf::from(socket);
    if !path.is_absolute() {
        return Err(HostError::InvalidGreetdSocket);
    }
    Ok(path)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostError {
    Configuration,
    ExternalTheme,
    MissingGreetdSocket,
    InvalidGreetdSocket,
    WorkerSpawn,
    SessionDiscovery,
    NoSessions,
    InvalidSessionCatalog,
    SharedView,
}

impl fmt::Display for HostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "the system configuration is invalid",
            Self::ExternalTheme => "the configured external theme is invalid",
            Self::MissingGreetdSocket => "GREETD_SOCK is not set",
            Self::InvalidGreetdSocket => "GREETD_SOCK must be a non-empty absolute path",
            Self::WorkerSpawn => "the authentication worker could not be started",
            Self::SessionDiscovery => "the trusted session catalog could not be discovered",
            Self::NoSessions => "the trusted session catalog is empty",
            Self::InvalidSessionCatalog => {
                "the trusted session catalog exceeds frontend safety limits"
            }
            Self::SharedView => "the shared WebKitGTK view could not be initialized",
        })
    }
}

impl Error for HostError {}
