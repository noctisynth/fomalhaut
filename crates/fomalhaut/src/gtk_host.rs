//! Native GTK4 and WebKitGTK 6.0 application host.

use std::{
    cell::{Cell, RefCell},
    env,
    error::Error,
    fmt,
    path::PathBuf,
    rc::Rc,
    sync::mpsc::{Receiver, TryRecvError},
    time::Duration,
};

use fomalhaut_session::{
    DiscoveryConfig, SessionDirectory, SessionKind as CatalogSessionKind, discover,
};
use fomalhaut_web::{
    assets::{EMBEDDED_THEME_CSP, EMBEDDED_THEME_HEADERS, resolve_builtin_asset},
    bridge::response_json,
    controller::TrustedSession,
    protocol::{
        MAX_SESSIONS, ProtocolErrorBody, ProtocolErrorCode, RequestId, ResponseEnvelope,
        SessionKind as WebSessionKind, SessionSummary, decode_request,
    },
};
use gtk4 as gtk;
use webkit6::{
    CacheModel, LoadEvent, NavigationPolicyDecision, NetworkSession, PolicyDecision,
    PolicyDecisionType, ResponsePolicyDecision, ScriptMessageReply, Settings, URISchemeRequest,
    URISchemeResponse, UserContentManager, WebContext, WebProcessTerminationReason, WebView,
    javascriptcore, soup,
};
use webkit6::{gio, glib, prelude::*};

use crate::controller_worker::{SubmitError, WorkerHandle, WorkerOutput};

const APPLICATION_ID: &str = "org.fomalhautdm.Fomalhaut";
const BRIDGE_NAME: &str = "fomalhaut";
const BUILTIN_THEME_URI: &str = "fomalhaut://theme/";
const NOT_FOUND_BODY: &[u8] = b"The requested embedded theme resource does not exist.\n";
const METHOD_NOT_ALLOWED_BODY: &[u8] = b"The embedded theme resource scheme only accepts GET.\n";
const CONTROLLER_POLL_INTERVAL: Duration = Duration::from_millis(10);
const DEFAULT_EXECUTABLE_DIRS: [&str; 2] = ["/usr/local/bin", "/usr/bin"];
const DEFAULT_SESSION_DIRS: [(&str, CatalogSessionKind); 4] = [
    (
        "/usr/local/share/wayland-sessions",
        CatalogSessionKind::Wayland,
    ),
    ("/usr/share/wayland-sessions", CatalogSessionKind::Wayland),
    ("/usr/local/share/xsessions", CatalogSessionKind::X11),
    ("/usr/share/xsessions", CatalogSessionKind::X11),
];

struct PendingReply {
    epoch: u64,
    context: javascriptcore::Context,
    reply: ScriptMessageReply,
}

/// Runs the native host until its GTK application exits.
pub fn run() -> glib::ExitCode {
    let application = gtk::Application::builder()
        .application_id(APPLICATION_ID)
        .build();
    let failed = Rc::new(Cell::new(false));
    let activation_failed = Rc::clone(&failed);
    application.connect_activate(move |application| activate(application, &activation_failed));
    let exit_code = application.run();
    if failed.get() {
        glib::ExitCode::FAILURE
    } else {
        exit_code
    }
}

fn activate(application: &gtk::Application, failed: &Rc<Cell<bool>>) {
    match build_window(application, Rc::clone(failed)) {
        Ok(window) => {
            window.fullscreen();
            window.present();
        }
        Err(error) => {
            eprintln!("Fomalhaut WebKitGTK host failed to initialize: {error}");
            failed.set(true);
            application.quit();
        }
    }
}

fn build_window(
    application: &gtk::Application,
    failed: Rc<Cell<bool>>,
) -> Result<gtk::ApplicationWindow, HostError> {
    let socket_path = greetd_socket_path()?;
    let sessions = discover_trusted_sessions()?;
    let (worker, outputs) =
        WorkerHandle::spawn(socket_path, sessions).map_err(|_| HostError::WorkerSpawn)?;
    let worker = Rc::new(worker);
    let page_epoch = Rc::new(Cell::new(0));
    let pending_reply = Rc::new(RefCell::new(None));

    let context = WebContext::new();
    context.set_automation_allowed(false);
    context.set_cache_model(CacheModel::DocumentViewer);
    context.set_spell_checking_enabled(false);
    register_scheme(&context)?;

    let content_manager = UserContentManager::new();
    connect_bridge(
        &content_manager,
        Rc::clone(&worker),
        Rc::clone(&page_epoch),
        Rc::clone(&pending_reply),
    )?;

    let network_session = NetworkSession::new_ephemeral();
    network_session.connect_download_started(|_, download| {
        download.cancel();
        eprintln!("Fomalhaut blocked a WebView download request");
    });

    let settings = secure_settings();
    let web_view = WebView::builder()
        .web_context(&context)
        .network_session(&network_session)
        .user_content_manager(&content_manager)
        .settings(&settings)
        .default_content_security_policy(EMBEDDED_THEME_CSP)
        .build();
    connect_worker_outputs(
        &web_view,
        application,
        outputs,
        Rc::clone(&worker),
        Rc::clone(&page_epoch),
        Rc::clone(&pending_reply),
        Rc::clone(&failed),
    );
    connect_web_view_policy(
        &web_view,
        application,
        Rc::clone(&worker),
        Rc::clone(&page_epoch),
        Rc::clone(&pending_reply),
        failed,
    );

    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title("Fomalhaut")
        .default_width(1280)
        .default_height(720)
        .child(&web_view)
        .build();
    window.connect_close_request(move |_| {
        eprintln!("Fomalhaut host window is closing");
        reject_pending_reply(&pending_reply, "the host window closed");
        worker.shutdown();
        glib::Propagation::Proceed
    });

    web_view.load_uri(BUILTIN_THEME_URI);
    Ok(window)
}

fn discover_trusted_sessions() -> Result<Vec<TrustedSession>, HostError> {
    let directories = DEFAULT_SESSION_DIRS
        .iter()
        .map(|(path, kind)| SessionDirectory::new(path, *kind))
        .collect();
    let executable_paths = DEFAULT_EXECUTABLE_DIRS.iter().map(PathBuf::from).collect();
    let config = DiscoveryConfig::new(directories).with_executable_search_paths(executable_paths);
    let report = discover(&config).map_err(|_| HostError::SessionDiscovery)?;
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

fn secure_settings() -> Settings {
    Settings::builder()
        .allow_file_access_from_file_urls(false)
        .allow_modal_dialogs(false)
        .allow_top_navigation_to_data_urls(false)
        .allow_universal_access_from_file_urls(false)
        .disable_web_security(false)
        .enable_developer_extras(false)
        .enable_dns_prefetching(false)
        .enable_encrypted_media(false)
        .enable_fullscreen(false)
        .enable_html5_database(false)
        .enable_html5_local_storage(false)
        .enable_javascript(true)
        .enable_media(false)
        .enable_media_capabilities(false)
        .enable_media_stream(false)
        .enable_mediasource(false)
        .enable_mock_capture_devices(false)
        .enable_offline_web_application_cache(false)
        .enable_page_cache(false)
        .enable_site_specific_quirks(false)
        .enable_webaudio(false)
        .enable_webgl(false)
        .enable_webrtc(false)
        .enable_write_console_messages_to_stdout(false)
        .javascript_can_access_clipboard(false)
        .javascript_can_open_windows_automatically(false)
        .build()
}

fn register_scheme(context: &WebContext) -> Result<(), HostError> {
    let security_manager = context
        .security_manager()
        .ok_or(HostError::MissingSecurityManager)?;
    security_manager.register_uri_scheme_as_secure("fomalhaut");
    security_manager.register_uri_scheme_as_display_isolated("fomalhaut");
    context.register_uri_scheme("fomalhaut", respond_to_scheme_request);
    Ok(())
}

fn respond_to_scheme_request(request: &URISchemeRequest) {
    if request.http_method().as_deref() != Some("GET") {
        finish_scheme_response(
            request,
            405,
            "Method Not Allowed",
            METHOD_NOT_ALLOWED_BODY,
            "text/plain; charset=utf-8",
        );
        return;
    }

    let asset = request.uri().as_deref().and_then(resolve_builtin_asset);
    match asset {
        Some(asset) => {
            eprintln!(
                "Fomalhaut served an allowlisted embedded theme resource ({})",
                asset.content_type()
            );
            finish_scheme_response(request, 200, "OK", asset.body(), asset.content_type())
        }
        None => finish_scheme_response(
            request,
            404,
            "Not Found",
            NOT_FOUND_BODY,
            "text/plain; charset=utf-8",
        ),
    }
}

fn finish_scheme_response(
    request: &URISchemeRequest,
    status: u32,
    reason: &str,
    body: &'static [u8],
    content_type: &str,
) {
    let bytes = glib::Bytes::from_static(body);
    let stream = gio::MemoryInputStream::from_bytes(&bytes);
    let length = i64::try_from(body.len())
        .expect("embedded theme resource lengths always fit within signed 64-bit integers");
    let response = URISchemeResponse::new(&stream, length);
    response.set_content_type(content_type);
    response.set_status(status, Some(reason));
    let headers = soup::MessageHeaders::new(soup::MessageHeadersType::Response);
    for (name, value) in EMBEDDED_THEME_HEADERS {
        headers.append(name, value);
    }
    response.set_http_headers(headers);
    request.finish_with_response(&response);
}

fn connect_bridge(
    content_manager: &UserContentManager,
    worker: Rc<WorkerHandle>,
    page_epoch: Rc<Cell<u64>>,
    pending_reply: Rc<RefCell<Option<PendingReply>>>,
) -> Result<(), HostError> {
    content_manager.connect_script_message_with_reply_received(
        Some(BRIDGE_NAME),
        move |_, value, reply| {
            eprintln!("Fomalhaut bridge received a script message");
            let Some(input) = value.to_json(0) else {
                eprintln!("Fomalhaut bridge rejected a non-JSON script value");
                reply.return_error_message("the bridge requires a JSON-compatible message");
                return true;
            };
            let request = match decode_request(input.as_bytes()) {
                Ok(request) => request,
                Err(error) => {
                    if let Some(id) = error.request_id() {
                        reply_with_response(
                            value,
                            reply,
                            ResponseEnvelope::error(id, error.body().clone()),
                        );
                    } else {
                        reply.return_error_message("the bridge rejected an invalid request");
                    }
                    return true;
                }
            };

            let id = request.id();
            if pending_reply.borrow().is_some() {
                reply_with_response(
                    value,
                    reply,
                    protocol_error_response(
                        id,
                        ProtocolErrorCode::InvalidState,
                        "another bridge request is still in progress",
                        true,
                    ),
                );
                return true;
            }
            let Some(context) = value.context() else {
                reply.return_error_message("the bridge could not access the script context");
                return true;
            };
            let epoch = page_epoch.get();
            match worker.submit(epoch, request) {
                Ok(()) => {
                    pending_reply.replace(Some(PendingReply {
                        epoch,
                        context,
                        reply: reply.clone(),
                    }));
                }
                Err(SubmitError::Busy) => reply_with_response(
                    value,
                    reply,
                    protocol_error_response(
                        id,
                        ProtocolErrorCode::Internal,
                        "the authentication controller is busy",
                        true,
                    ),
                ),
                Err(SubmitError::Stopped) => {
                    reply.return_error_message("the authentication controller is unavailable")
                }
            }
            true
        },
    );

    if !content_manager.register_script_message_handler_with_reply(BRIDGE_NAME, None) {
        return Err(HostError::BridgeRegistration);
    }
    Ok(())
}

fn protocol_error_response(
    id: RequestId,
    code: ProtocolErrorCode,
    message: &'static str,
    retryable: bool,
) -> ResponseEnvelope {
    ResponseEnvelope::error(id, ProtocolErrorBody::new(code, message, retryable))
}

fn reply_with_response(
    value: &javascriptcore::Value,
    reply: &ScriptMessageReply,
    response: ResponseEnvelope,
) {
    let Some(context) = value.context() else {
        reply.return_error_message("the bridge could not access the script context");
        return;
    };
    let json = match response_json(&response) {
        Ok(json) => json,
        Err(_) => {
            reply.return_error_message("the bridge could not encode a protocol response");
            return;
        }
    };
    let response = javascriptcore::Value::from_json(&context, &json);
    reply.return_value(&response);
}

fn reject_pending_reply(pending_reply: &RefCell<Option<PendingReply>>, message: &str) {
    if let Some(pending) = pending_reply.borrow_mut().take() {
        pending.reply.return_error_message(message);
    }
}

fn connect_worker_outputs(
    web_view: &WebView,
    application: &gtk::Application,
    outputs: Receiver<WorkerOutput>,
    worker: Rc<WorkerHandle>,
    page_epoch: Rc<Cell<u64>>,
    pending_reply: Rc<RefCell<Option<PendingReply>>>,
    failed: Rc<Cell<bool>>,
) {
    let web_view = web_view.downgrade();
    let application = application.downgrade();
    glib::timeout_add_local(CONTROLLER_POLL_INTERVAL, move || {
        loop {
            match outputs.try_recv() {
                Ok(WorkerOutput::Ready) => {
                    eprintln!("Fomalhaut authentication controller connected to greetd")
                }
                Ok(WorkerOutput::Batch(batch)) => {
                    if batch.epoch != page_epoch.get() {
                        eprintln!("Fomalhaut discarded output from a stale page context");
                        if batch.session_started {
                            worker.shutdown();
                            if let Some(application) = application.upgrade() {
                                application.quit();
                            }
                            return glib::ControlFlow::Break;
                        }
                        continue;
                    }
                    let Some(pending) = pending_reply.borrow_mut().take() else {
                        eprintln!("Fomalhaut controller produced an uncorrelated reply");
                        failed.set(true);
                        worker.shutdown();
                        if let Some(application) = application.upgrade() {
                            application.quit();
                        }
                        return glib::ControlFlow::Break;
                    };
                    if pending.epoch != batch.epoch {
                        pending
                            .reply
                            .return_error_message("the page context changed before reply delivery");
                        continue;
                    }
                    let response =
                        javascriptcore::Value::from_json(&pending.context, &batch.response);
                    pending.reply.return_value(&response);

                    if let Some(web_view) = web_view.upgrade() {
                        for script in batch.event_scripts {
                            web_view.evaluate_javascript(
                                &script,
                                None,
                                None,
                                None::<&gio::Cancellable>,
                                |result| {
                                    if result.is_err() {
                                        eprintln!("Fomalhaut failed to deliver a controller event");
                                    }
                                },
                            );
                        }
                    }
                    if batch.session_started {
                        eprintln!("Fomalhaut trusted user session started; host is exiting");
                        worker.shutdown();
                        if let Some(application) = application.upgrade() {
                            application.quit();
                        }
                        return glib::ControlFlow::Break;
                    }
                }
                Ok(WorkerOutput::Fatal(message)) => {
                    eprintln!("Fomalhaut authentication controller failed: {message}");
                    failed.set(true);
                    reject_pending_reply(&pending_reply, "the authentication controller stopped");
                    worker.shutdown();
                    if let Some(application) = application.upgrade() {
                        application.quit();
                    }
                    return glib::ControlFlow::Break;
                }
                Err(TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => {
                    eprintln!("Fomalhaut authentication controller output channel closed");
                    failed.set(true);
                    reject_pending_reply(
                        &pending_reply,
                        "the authentication controller disconnected",
                    );
                    worker.shutdown();
                    if let Some(application) = application.upgrade() {
                        application.quit();
                    }
                    return glib::ControlFlow::Break;
                }
            }
        }
    });
}

fn connect_web_view_policy(
    web_view: &WebView,
    application: &gtk::Application,
    worker: Rc<WorkerHandle>,
    page_epoch: Rc<Cell<u64>>,
    pending_reply: Rc<RefCell<Option<PendingReply>>>,
    failed: Rc<Cell<bool>>,
) {
    web_view.connect_create(|_, _| {
        eprintln!("Fomalhaut blocked a WebView new-window request");
        None
    });
    web_view.connect_permission_request(|_, request| {
        request.deny();
        eprintln!("Fomalhaut denied a WebView permission request");
        true
    });
    web_view.connect_decide_policy(|_, decision, decision_type| {
        decide_policy(decision, decision_type);
        true
    });

    let terminated_application = application.downgrade();
    let terminated_worker = Rc::clone(&worker);
    let terminated_pending = Rc::clone(&pending_reply);
    let terminated_failed = Rc::clone(&failed);
    web_view.connect_web_process_terminated(move |_, reason| {
        report_web_process_termination(reason);
        terminated_failed.set(true);
        reject_pending_reply(&terminated_pending, "the WebView renderer terminated");
        terminated_worker.shutdown();
        if let Some(application) = terminated_application.upgrade() {
            application.quit();
        }
    });

    web_view.connect_load_failed(|_, _, _, _| {
        eprintln!("Fomalhaut WebView failed to load an allowed resource");
        true
    });
    let load_application = application.downgrade();
    web_view.connect_load_changed(move |_, event| match event {
        LoadEvent::Started => {
            let Some(next_epoch) = page_epoch.get().checked_add(1) else {
                eprintln!("Fomalhaut page epoch exhausted");
                failed.set(true);
                reject_pending_reply(&pending_reply, "the page epoch is exhausted");
                worker.shutdown();
                if let Some(application) = load_application.upgrade() {
                    application.quit();
                }
                return;
            };
            page_epoch.set(next_epoch);
            reject_pending_reply(
                &pending_reply,
                "the page context changed before reply delivery",
            );
            if worker.cancel_for_page().is_err() {
                eprintln!("Fomalhaut could not queue page authentication cancellation");
                failed.set(true);
                worker.shutdown();
                if let Some(application) = load_application.upgrade() {
                    application.quit();
                }
                return;
            }
            eprintln!("Fomalhaut invalidated the previous page context before loading");
        }
        LoadEvent::Redirected => eprintln!("Fomalhaut observed a WebView redirect"),
        LoadEvent::Committed => eprintln!("Fomalhaut committed an allowlisted page load"),
        LoadEvent::Finished => eprintln!("Fomalhaut finished loading the current page context"),
        _ => eprintln!("Fomalhaut observed an unknown WebView load transition"),
    });
}

fn decide_policy(decision: &PolicyDecision, decision_type: PolicyDecisionType) {
    let allowed = match decision_type {
        PolicyDecisionType::NavigationAction => navigation_is_allowed(decision),
        PolicyDecisionType::Response => response_is_allowed(decision),
        PolicyDecisionType::NewWindowAction => false,
        _ => false,
    };
    if allowed {
        decision.use_();
    } else {
        decision.ignore();
        eprintln!("Fomalhaut blocked a WebView policy decision");
    }
}

fn navigation_is_allowed(decision: &PolicyDecision) -> bool {
    decision
        .downcast_ref::<NavigationPolicyDecision>()
        .and_then(NavigationPolicyDecision::navigation_action)
        .and_then(|action| action.request())
        .and_then(|request| request.uri())
        .as_deref()
        .and_then(resolve_builtin_asset)
        .is_some()
}

fn response_is_allowed(decision: &PolicyDecision) -> bool {
    let Some(response) = decision.downcast_ref::<ResponsePolicyDecision>() else {
        return false;
    };
    response.is_mime_type_supported()
        && response
            .request()
            .and_then(|request| request.uri())
            .as_deref()
            .and_then(resolve_builtin_asset)
            .is_some()
}

fn report_web_process_termination(reason: WebProcessTerminationReason) {
    eprintln!("Fomalhaut WebView renderer terminated: {reason:?}");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostError {
    MissingGreetdSocket,
    InvalidGreetdSocket,
    WorkerSpawn,
    SessionDiscovery,
    NoSessions,
    InvalidSessionCatalog,
    MissingSecurityManager,
    BridgeRegistration,
}

impl fmt::Display for HostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingGreetdSocket => "GREETD_SOCK is not set",
            Self::InvalidGreetdSocket => "GREETD_SOCK must be a non-empty absolute path",
            Self::WorkerSpawn => "the authentication worker could not be started",
            Self::SessionDiscovery => "the trusted session catalog could not be discovered",
            Self::NoSessions => "the trusted session catalog is empty",
            Self::InvalidSessionCatalog => {
                "the trusted session catalog exceeds frontend safety limits"
            }
            Self::MissingSecurityManager => "WebKit did not provide a security manager",
            Self::BridgeRegistration => "the JavaScript message handler could not be registered",
        })
    }
}

impl Error for HostError {}
