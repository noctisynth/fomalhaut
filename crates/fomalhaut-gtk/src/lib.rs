//! Shared GTK4 and WebKitGTK host infrastructure for Fomalhaut.

use std::{
    cell::{Cell, RefCell},
    error::Error,
    fmt,
    rc::Rc,
    sync::mpsc::{Receiver, TryRecvError},
    time::Duration,
};

use fomalhaut_web::{
    assets::{THEME_CSP, THEME_HEADERS},
    bridge::response_json,
    protocol::{
        ProtocolErrorBody, ProtocolErrorCode, RequestEnvelope, RequestId, ResponseEnvelope,
        RuntimeMode, decode_request_for_mode,
    },
    theme::ThemeSource,
};
use gtk4 as gtk;
use webkit6::{
    CacheModel, LoadEvent, NavigationPolicyDecision, NetworkSession, PolicyDecision,
    PolicyDecisionType, ResponsePolicyDecision, ScriptMessageReply, Settings, URISchemeRequest,
    URISchemeResponse, UserContentManager, WebContext, WebProcessTerminationReason, WebView,
    javascriptcore, soup,
};
use webkit6::{gio, glib, prelude::*};

const BRIDGE_NAME: &str = "fomalhaut";
const BUILTIN_THEME_URI: &str = "fomalhaut://theme/";
const NOT_FOUND_BODY: &[u8] = b"The requested theme resource does not exist.\n";
const METHOD_NOT_ALLOWED_BODY: &[u8] = b"The theme resource scheme only accepts GET.\n";
const RESOURCE_ERROR_BODY: &[u8] = b"The requested theme resource could not be loaded.\n";
const CONTROLLER_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_AVATAR_RESOURCE_BYTES: usize = 2 * 1024 * 1024;

/// Non-blocking bridge submission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitError {
    /// The bounded controller queue cannot currently accept another request.
    Busy,
    /// The controller worker is no longer available.
    Stopped,
}

/// Role-neutral controller capability consumed by the GTK main thread.
pub trait BridgeController {
    /// Queues one strictly decoded frontend request for the current page epoch.
    fn submit(&self, epoch: u64, request: RequestEnvelope) -> Result<(), SubmitError>;

    /// Queues cancellation of authentication owned by the previous page.
    fn cancel_for_page(&self) -> Result<(), SubmitError>;

    /// Stops the controller and waits for its worker to finish.
    fn shutdown(&self);
}

/// Serialized controller output delivered atomically to one WebView page.
pub struct ControllerBatch<A> {
    /// Page epoch that originated the request.
    pub epoch: u64,
    /// Serialized protocol response.
    pub response: String,
    /// Serialized event-dispatch scripts in sequence order.
    pub event_scripts: Vec<String>,
    /// Optional role-specific terminal action owned by the native host.
    pub terminal: Option<A>,
}

/// Output from a role-specific controller worker.
pub enum ControllerOutput<A> {
    /// The controller is ready and supplies validated auxiliary resources.
    Ready(Vec<ResourceAsset>),
    /// One response/event transaction completed.
    Batch(ControllerBatch<A>),
    /// Unsolicited native or cross-view events for the current page.
    Events(Vec<String>),
    /// The controller stopped because a sanitized fatal error occurred.
    Fatal(&'static str),
}

/// Validated in-memory resource served by the native URI scheme.
pub struct ResourceAsset {
    uri: String,
    body: Vec<u8>,
    content_type: &'static str,
}

impl ResourceAsset {
    /// Constructs an opaque avatar resource from trusted, already bounded bytes.
    pub fn avatar(
        identifier: usize,
        body: Vec<u8>,
        content_type: &'static str,
    ) -> Result<Self, ResourceAssetError> {
        if identifier == 0 {
            return Err(ResourceAssetError::InvalidIdentifier);
        }
        if body.len() > MAX_AVATAR_RESOURCE_BYTES {
            return Err(ResourceAssetError::TooLarge);
        }
        if !matches!(content_type, "image/png" | "image/jpeg" | "image/webp") {
            return Err(ResourceAssetError::InvalidContentType);
        }
        Ok(Self {
            uri: format!("fomalhaut://avatar/{identifier}"),
            body,
            content_type,
        })
    }

    /// Returns the exact opaque URI exposed in a public user summary.
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    fn response_body(&self) -> Vec<u8> {
        self.body.clone()
    }

    /// Returns the allowlisted media type selected by trusted native code.
    #[must_use]
    pub const fn content_type(&self) -> &'static str {
        self.content_type
    }

    fn matches_uri(&self, uri: &str) -> bool {
        self.uri == uri
    }
}

/// Invalid trusted auxiliary resource metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceAssetError {
    /// Resource identifiers start at one.
    InvalidIdentifier,
    /// Avatar resources are limited to 2 MiB.
    TooLarge,
    /// Only image types recognized by the avatar sniffer are accepted.
    InvalidContentType,
}

impl fmt::Display for ResourceAssetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidIdentifier => "the resource identifier must be non-zero",
            Self::TooLarge => "the avatar resource exceeds 2 MiB",
            Self::InvalidContentType => "the resource content type is not allowlisted",
        })
    }
}

impl Error for ResourceAssetError {}

/// A failure that requires the product host to apply its role-specific policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewFailure {
    /// The controller reported a sanitized fatal error.
    ControllerFatal(&'static str),
    /// The controller output channel disconnected unexpectedly.
    ControllerDisconnected,
    /// The controller produced a response without a matching pending request.
    UncorrelatedReply,
    /// The WebKit renderer process terminated.
    RendererTerminated,
    /// The bounded page epoch counter was exhausted.
    PageEpochExhausted,
    /// Cancellation for a replaced page could not be queued.
    PageCancellation,
}

impl fmt::Display for ViewFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ControllerFatal(message) => message,
            Self::ControllerDisconnected => "the authentication controller disconnected",
            Self::UncorrelatedReply => "the controller produced an uncorrelated reply",
            Self::RendererTerminated => "the WebView renderer terminated",
            Self::PageEpochExhausted => "the page epoch was exhausted",
            Self::PageCancellation => "stale page authentication could not be cancelled",
        })
    }
}

impl Error for ViewFailure {}

/// Role-owned reactions to shared WebView lifecycle events.
pub struct ViewCallbacks<A> {
    on_ready: Box<dyn Fn()>,
    on_terminal: Box<dyn Fn(A)>,
    on_failure: Box<dyn Fn(ViewFailure)>,
}

impl<A> ViewCallbacks<A> {
    /// Constructs callbacks invoked on the GTK main thread.
    pub fn new(
        on_ready: impl Fn() + 'static,
        on_terminal: impl Fn(A) + 'static,
        on_failure: impl Fn(ViewFailure) + 'static,
    ) -> Self {
        Self {
            on_ready: Box::new(on_ready),
            on_terminal: Box::new(on_terminal),
            on_failure: Box::new(on_failure),
        }
    }
}

struct PendingReply {
    epoch: u64,
    context: javascriptcore::Context,
    reply: ScriptMessageReply,
}

struct ControllerOutputContext<C, A> {
    controller: Rc<C>,
    page_epoch: Rc<Cell<u64>>,
    pending_reply: Rc<RefCell<Option<PendingReply>>>,
    stopped: Rc<Cell<bool>>,
    resources: Rc<RefCell<Vec<ResourceAsset>>>,
    callbacks: Rc<ViewCallbacks<A>>,
}

/// Role-neutral handle for the shared GTK application lifecycle.
#[derive(Clone)]
pub struct ApplicationHandle {
    application: gtk::Application,
    failed: Rc<Cell<bool>>,
}

impl ApplicationHandle {
    /// Returns the GTK application used to construct role-owned windows.
    #[must_use]
    pub const fn application(&self) -> &gtk::Application {
        &self.application
    }

    /// Requests a successful application exit.
    pub fn quit(&self) {
        self.application.quit();
    }

    /// Marks the product run as failed and requests application exit.
    pub fn quit_failure(&self) {
        self.failed.set(true);
        self.application.quit();
    }
}

/// Runs a shared GTK application while leaving window composition to the product role.
pub fn run_application<E>(
    application_id: &str,
    activate: impl Fn(ApplicationHandle) -> Result<(), E> + 'static,
) -> glib::ExitCode
where
    E: fmt::Display + 'static,
{
    let application = gtk::Application::builder()
        .application_id(application_id)
        .build();
    let failed = Rc::new(Cell::new(false));
    let handle = ApplicationHandle {
        application: application.clone(),
        failed: Rc::clone(&failed),
    };
    application.connect_activate(move |_| {
        if let Err(error) = activate(handle.clone()) {
            eprintln!("Fomalhaut GTK application activation failed: {error}");
            handle.quit_failure();
        }
    });
    let exit_code = application.run();
    if failed.get() {
        glib::ExitCode::FAILURE
    } else {
        exit_code
    }
}

/// Shared WebView plus the controller lifecycle it owns.
pub struct HostedView<C> {
    web_view: WebView,
    controller: Rc<C>,
    pending_reply: Rc<RefCell<Option<PendingReply>>>,
    stopped: Rc<Cell<bool>>,
}

impl<C> Clone for HostedView<C> {
    fn clone(&self) -> Self {
        Self {
            web_view: self.web_view.clone(),
            controller: Rc::clone(&self.controller),
            pending_reply: Rc::clone(&self.pending_reply),
            stopped: Rc::clone(&self.stopped),
        }
    }
}

impl<C: BridgeController> HostedView<C> {
    /// Returns the WebView widget for placement in a role-owned GTK window.
    #[must_use]
    pub const fn web_view(&self) -> &WebView {
        &self.web_view
    }

    /// Loads the selected theme entrypoint.
    pub fn load_theme(&self) {
        self.web_view.load_uri(BUILTIN_THEME_URI);
    }

    /// Stops bridge activity and waits for the controller worker to finish.
    pub fn shutdown(&self) {
        if self.stopped.replace(true) {
            return;
        }
        reject_pending_reply(&self.pending_reply, "the native host stopped");
        self.controller.shutdown();
    }
}

/// Builds a hardened WebView while leaving window and product lifecycle ownership to the caller.
pub fn build_web_view<C, A>(
    theme: ThemeSource,
    mode: RuntimeMode,
    zoom_level: f64,
    controller: Rc<C>,
    outputs: Receiver<ControllerOutput<A>>,
    callbacks: ViewCallbacks<A>,
) -> Result<HostedView<C>, ViewBuildError>
where
    C: BridgeController + 'static,
    A: 'static,
{
    let theme = Rc::new(theme);
    let resources = Rc::new(RefCell::new(Vec::new()));
    let page_epoch = Rc::new(Cell::new(0));
    let pending_reply = Rc::new(RefCell::new(None));
    let stopped = Rc::new(Cell::new(false));
    let callbacks = Rc::new(callbacks);

    let context = WebContext::new();
    context.set_automation_allowed(false);
    context.set_cache_model(CacheModel::DocumentViewer);
    context.set_spell_checking_enabled(false);
    register_scheme(&context, Rc::clone(&theme), Rc::clone(&resources))?;

    let content_manager = UserContentManager::new();
    connect_bridge(
        &content_manager,
        mode,
        Rc::clone(&controller),
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
        .zoom_level(zoom_level)
        .default_content_security_policy(THEME_CSP)
        .build();
    connect_controller_outputs(
        &web_view,
        outputs,
        ControllerOutputContext {
            controller: Rc::clone(&controller),
            page_epoch: Rc::clone(&page_epoch),
            pending_reply: Rc::clone(&pending_reply),
            stopped: Rc::clone(&stopped),
            resources,
            callbacks: Rc::clone(&callbacks),
        },
    );
    connect_web_view_policy(
        &web_view,
        Rc::clone(&controller),
        Rc::clone(&page_epoch),
        Rc::clone(&pending_reply),
        theme,
        Rc::clone(&stopped),
        callbacks,
    );

    Ok(HostedView {
        web_view,
        controller,
        pending_reply,
        stopped: Rc::clone(&stopped),
    })
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

fn register_scheme(
    context: &WebContext,
    theme: Rc<ThemeSource>,
    resources: Rc<RefCell<Vec<ResourceAsset>>>,
) -> Result<(), ViewBuildError> {
    let security_manager = context
        .security_manager()
        .ok_or(ViewBuildError::MissingSecurityManager)?;
    security_manager.register_uri_scheme_as_secure("fomalhaut");
    security_manager.register_uri_scheme_as_display_isolated("fomalhaut");
    context.register_uri_scheme("fomalhaut", move |request| {
        respond_to_scheme_request(request, &theme, &resources.borrow());
    });
    Ok(())
}

fn respond_to_scheme_request(
    request: &URISchemeRequest,
    theme: &ThemeSource,
    resources: &[ResourceAsset],
) {
    if request.http_method().as_deref() != Some("GET") {
        finish_scheme_response(
            request,
            405,
            "Method Not Allowed",
            METHOD_NOT_ALLOWED_BODY.to_vec(),
            "text/plain; charset=utf-8",
        );
        return;
    }

    let uri = request.uri();
    if let Some(resource) = uri
        .as_deref()
        .and_then(|uri| resources.iter().find(|asset| asset.matches_uri(uri)))
    {
        finish_scheme_response(
            request,
            200,
            "OK",
            resource.response_body(),
            resource.content_type(),
        );
        return;
    }

    let asset = uri.as_deref().map(|uri| theme.resolve(uri));
    match asset {
        Some(Ok(Some(asset))) => {
            let (body, content_type) = asset.into_parts();
            eprintln!("Fomalhaut served an allowlisted theme resource ({content_type})");
            finish_scheme_response(request, 200, "OK", body, content_type)
        }
        Some(Err(_)) => {
            eprintln!("Fomalhaut could not read an allowlisted theme resource");
            finish_scheme_response(
                request,
                500,
                "Internal Server Error",
                RESOURCE_ERROR_BODY.to_vec(),
                "text/plain; charset=utf-8",
            )
        }
        Some(Ok(None)) | None => finish_scheme_response(
            request,
            404,
            "Not Found",
            NOT_FOUND_BODY.to_vec(),
            "text/plain; charset=utf-8",
        ),
    }
}

fn finish_scheme_response(
    request: &URISchemeRequest,
    status: u32,
    reason: &str,
    body: Vec<u8>,
    content_type: &'static str,
) {
    let length = i64::try_from(body.len()).expect(
        "theme resources and fixed error bodies are bounded far below signed 64-bit lengths",
    );
    let bytes = glib::Bytes::from_owned(body);
    let stream = gio::MemoryInputStream::from_bytes(&bytes);
    let response = URISchemeResponse::new(&stream, length);
    response.set_content_type(content_type);
    response.set_status(status, Some(reason));
    let headers = soup::MessageHeaders::new(soup::MessageHeadersType::Response);
    for (name, value) in THEME_HEADERS {
        headers.append(name, value);
    }
    response.set_http_headers(headers);
    request.finish_with_response(&response);
}

fn connect_bridge<C: BridgeController + 'static>(
    content_manager: &UserContentManager,
    mode: RuntimeMode,
    controller: Rc<C>,
    page_epoch: Rc<Cell<u64>>,
    pending_reply: Rc<RefCell<Option<PendingReply>>>,
) -> Result<(), ViewBuildError> {
    content_manager.connect_script_message_with_reply_received(
        Some(BRIDGE_NAME),
        move |_, value, reply| {
            eprintln!("Fomalhaut bridge received a script message");
            let Some(input) = value.to_json(0) else {
                eprintln!("Fomalhaut bridge rejected a non-JSON script value");
                reply.return_error_message("the bridge requires a JSON-compatible message");
                return true;
            };
            let request = match decode_request_for_mode(input.as_bytes(), mode) {
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
            match controller.submit(epoch, request) {
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
        return Err(ViewBuildError::BridgeRegistration);
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

fn connect_controller_outputs<C, A>(
    web_view: &WebView,
    outputs: Receiver<ControllerOutput<A>>,
    context: ControllerOutputContext<C, A>,
) where
    C: BridgeController + 'static,
    A: 'static,
{
    let ControllerOutputContext {
        controller,
        page_epoch,
        pending_reply,
        stopped,
        resources,
        callbacks,
    } = context;
    let web_view = web_view.downgrade();
    glib::timeout_add_local(CONTROLLER_POLL_INTERVAL, move || {
        if stopped.get() {
            return glib::ControlFlow::Break;
        }
        loop {
            match outputs.try_recv() {
                Ok(ControllerOutput::Ready(discovered_resources)) => {
                    resources.replace(discovered_resources);
                    (callbacks.on_ready)();
                }
                Ok(ControllerOutput::Batch(batch)) => {
                    if batch.epoch != page_epoch.get() {
                        eprintln!("Fomalhaut discarded output from a stale page context");
                        if let Some(terminal) = batch.terminal {
                            finish_terminal(
                                terminal,
                                controller.as_ref(),
                                &pending_reply,
                                &stopped,
                                &callbacks,
                            );
                            return glib::ControlFlow::Break;
                        }
                        continue;
                    }
                    let Some(pending) = pending_reply.borrow_mut().take() else {
                        eprintln!("Fomalhaut controller produced an uncorrelated reply");
                        fail_host(
                            ViewFailure::UncorrelatedReply,
                            controller.as_ref(),
                            &pending_reply,
                            &stopped,
                            &callbacks,
                        );
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
                        deliver_event_scripts(&web_view, batch.event_scripts);
                    }
                    if let Some(terminal) = batch.terminal {
                        finish_terminal(
                            terminal,
                            controller.as_ref(),
                            &pending_reply,
                            &stopped,
                            &callbacks,
                        );
                        return glib::ControlFlow::Break;
                    }
                }
                Ok(ControllerOutput::Events(event_scripts)) => {
                    if let Some(web_view) = web_view.upgrade() {
                        deliver_event_scripts(&web_view, event_scripts);
                    }
                }
                Ok(ControllerOutput::Fatal(message)) => {
                    eprintln!("Fomalhaut authentication controller failed: {message}");
                    fail_host(
                        ViewFailure::ControllerFatal(message),
                        controller.as_ref(),
                        &pending_reply,
                        &stopped,
                        &callbacks,
                    );
                    return glib::ControlFlow::Break;
                }
                Err(TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => {
                    eprintln!("Fomalhaut authentication controller output channel closed");
                    fail_host(
                        ViewFailure::ControllerDisconnected,
                        controller.as_ref(),
                        &pending_reply,
                        &stopped,
                        &callbacks,
                    );
                    return glib::ControlFlow::Break;
                }
            }
        }
    });
}

fn deliver_event_scripts(web_view: &WebView, event_scripts: Vec<String>) {
    for script in event_scripts {
        web_view.evaluate_javascript(&script, None, None, None::<&gio::Cancellable>, |result| {
            if result.is_err() {
                eprintln!("Fomalhaut failed to deliver a controller event");
            }
        });
    }
}

fn finish_terminal<C: BridgeController, A>(
    terminal: A,
    controller: &C,
    pending_reply: &RefCell<Option<PendingReply>>,
    stopped: &Cell<bool>,
    callbacks: &ViewCallbacks<A>,
) {
    if stopped.replace(true) {
        return;
    }
    reject_pending_reply(pending_reply, "the native host completed its lifecycle");
    controller.shutdown();
    (callbacks.on_terminal)(terminal);
}

fn fail_host<C: BridgeController, A>(
    failure: ViewFailure,
    controller: &C,
    pending_reply: &RefCell<Option<PendingReply>>,
    stopped: &Cell<bool>,
    callbacks: &ViewCallbacks<A>,
) {
    if stopped.replace(true) {
        return;
    }
    reject_pending_reply(pending_reply, "the native host encountered a fatal error");
    controller.shutdown();
    (callbacks.on_failure)(failure);
}

fn connect_web_view_policy<C, A>(
    web_view: &WebView,
    controller: Rc<C>,
    page_epoch: Rc<Cell<u64>>,
    pending_reply: Rc<RefCell<Option<PendingReply>>>,
    theme: Rc<ThemeSource>,
    stopped: Rc<Cell<bool>>,
    callbacks: Rc<ViewCallbacks<A>>,
) where
    C: BridgeController + 'static,
    A: 'static,
{
    web_view.connect_create(|_, _| {
        eprintln!("Fomalhaut blocked a WebView new-window request");
        None
    });
    web_view.connect_permission_request(|_, request| {
        request.deny();
        eprintln!("Fomalhaut denied a WebView permission request");
        true
    });
    web_view.connect_decide_policy(move |_, decision, decision_type| {
        decide_policy(decision, decision_type, &theme);
        true
    });

    let terminated_controller = Rc::clone(&controller);
    let terminated_pending = Rc::clone(&pending_reply);
    let terminated_stopped = Rc::clone(&stopped);
    let terminated_callbacks = Rc::clone(&callbacks);
    web_view.connect_web_process_terminated(move |_, reason| {
        report_web_process_termination(reason);
        fail_host(
            ViewFailure::RendererTerminated,
            terminated_controller.as_ref(),
            &terminated_pending,
            &terminated_stopped,
            &terminated_callbacks,
        );
    });

    web_view.connect_load_failed(|_, _, _, _| {
        eprintln!("Fomalhaut WebView failed to load an allowed resource");
        true
    });
    web_view.connect_load_changed(move |_, event| match event {
        LoadEvent::Started => {
            if stopped.get() {
                return;
            }
            let Some(next_epoch) = page_epoch.get().checked_add(1) else {
                eprintln!("Fomalhaut page epoch exhausted");
                fail_host(
                    ViewFailure::PageEpochExhausted,
                    controller.as_ref(),
                    &pending_reply,
                    &stopped,
                    &callbacks,
                );
                return;
            };
            page_epoch.set(next_epoch);
            reject_pending_reply(
                &pending_reply,
                "the page context changed before reply delivery",
            );
            if controller.cancel_for_page().is_err() {
                eprintln!("Fomalhaut could not queue page authentication cancellation");
                fail_host(
                    ViewFailure::PageCancellation,
                    controller.as_ref(),
                    &pending_reply,
                    &stopped,
                    &callbacks,
                );
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

fn decide_policy(
    decision: &PolicyDecision,
    decision_type: PolicyDecisionType,
    theme: &ThemeSource,
) {
    let allowed = match decision_type {
        PolicyDecisionType::NavigationAction => navigation_is_allowed(decision, theme),
        PolicyDecisionType::Response => response_is_allowed(decision, theme),
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

fn navigation_is_allowed(decision: &PolicyDecision, theme: &ThemeSource) -> bool {
    decision
        .downcast_ref::<NavigationPolicyDecision>()
        .and_then(NavigationPolicyDecision::navigation_action)
        .and_then(|action| action.request())
        .and_then(|request| request.uri())
        .as_deref()
        .is_some_and(|uri| theme.allows_navigation(uri))
}

fn response_is_allowed(decision: &PolicyDecision, theme: &ThemeSource) -> bool {
    let Some(response) = decision.downcast_ref::<ResponsePolicyDecision>() else {
        return false;
    };
    response.is_mime_type_supported()
        && response
            .request()
            .and_then(|request| request.uri())
            .as_deref()
            .is_some_and(|uri| theme.allows_resource_uri(uri))
}

fn report_web_process_termination(reason: WebProcessTerminationReason) {
    eprintln!("Fomalhaut WebView renderer terminated: {reason:?}");
}

/// Failure while constructing shared WebKitGTK host infrastructure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewBuildError {
    /// WebKit did not provide the scheme security manager.
    MissingSecurityManager,
    /// The single JavaScript bridge handler could not be registered.
    BridgeRegistration,
}

impl fmt::Display for ViewBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingSecurityManager => "WebKit did not provide a security manager",
            Self::BridgeRegistration => "the JavaScript message handler could not be registered",
        })
    }
}

impl Error for ViewBuildError {}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use fomalhaut_web::protocol::RequestEnvelope;

    use super::{
        BridgeController, MAX_AVATAR_RESOURCE_BYTES, ResourceAsset, ResourceAssetError,
        SubmitError, ViewCallbacks, ViewFailure, fail_host, finish_terminal,
    };

    struct FakeController {
        shutdowns: Cell<usize>,
    }

    impl BridgeController for FakeController {
        fn submit(&self, _: u64, _: RequestEnvelope) -> Result<(), SubmitError> {
            Err(SubmitError::Stopped)
        }

        fn cancel_for_page(&self) -> Result<(), SubmitError> {
            Err(SubmitError::Stopped)
        }

        fn shutdown(&self) {
            self.shutdowns.set(self.shutdowns.get() + 1);
        }
    }

    #[test]
    fn avatar_resources_receive_opaque_host_uris() {
        let asset = ResourceAsset::avatar(7, vec![1, 2, 3], "image/png")
            .expect("allowlisted avatar metadata is accepted");

        assert_eq!(asset.uri(), "fomalhaut://avatar/7");
        assert!(asset.matches_uri("fomalhaut://avatar/7"));
        assert!(!asset.matches_uri("fomalhaut://avatar/8"));
        assert_eq!(asset.response_body(), [1, 2, 3]);
    }

    #[test]
    fn avatar_resources_reject_untrusted_metadata() {
        assert_eq!(
            ResourceAsset::avatar(0, vec![1], "image/png").err(),
            Some(ResourceAssetError::InvalidIdentifier)
        );
        assert_eq!(
            ResourceAsset::avatar(1, vec![1], "image/svg+xml").err(),
            Some(ResourceAssetError::InvalidContentType)
        );
        assert_eq!(
            ResourceAsset::avatar(1, vec![0; MAX_AVATAR_RESOURCE_BYTES + 1], "image/png").err(),
            Some(ResourceAssetError::TooLarge)
        );
    }

    #[test]
    fn terminal_actions_stop_the_controller_exactly_once() {
        let controller = FakeController {
            shutdowns: Cell::new(0),
        };
        let stopped = Cell::new(false);
        let pending = Default::default();
        let observed = Rc::new(Cell::new(None));
        let terminal_observed = Rc::clone(&observed);
        let callbacks = ViewCallbacks::new(
            || {},
            move |terminal| terminal_observed.set(Some(terminal)),
            |_| {},
        );

        finish_terminal(7, &controller, &pending, &stopped, &callbacks);
        finish_terminal(8, &controller, &pending, &stopped, &callbacks);

        assert!(stopped.get());
        assert_eq!(controller.shutdowns.get(), 1);
        assert_eq!(observed.get(), Some(7));
    }

    #[test]
    fn failures_stop_the_controller_and_preserve_the_first_reason() {
        let controller = FakeController {
            shutdowns: Cell::new(0),
        };
        let stopped = Cell::new(false);
        let pending = Default::default();
        let observed = Rc::new(Cell::new(None));
        let failure_observed = Rc::clone(&observed);
        let callbacks = ViewCallbacks::new(
            || {},
            |_: ()| {},
            move |failure| failure_observed.set(Some(failure)),
        );

        fail_host(
            ViewFailure::RendererTerminated,
            &controller,
            &pending,
            &stopped,
            &callbacks,
        );
        fail_host(
            ViewFailure::ControllerDisconnected,
            &controller,
            &pending,
            &stopped,
            &callbacks,
        );

        assert!(stopped.get());
        assert_eq!(controller.shutdowns.get(), 1);
        assert_eq!(observed.get(), Some(ViewFailure::RendererTerminated));
    }
}
