//! Native GTK4 and WebKitGTK 6.0 application host prototype.

use std::{error::Error, fmt};

use fomalhaut_web::{
    assets::{PROTOTYPE_CSP, PROTOTYPE_HEADERS, resolve_builtin_asset},
    prototype::{PrototypeReply, handle_prototype_request, prototype_event_dispatch_script},
};
use gtk4 as gtk;
use webkit6::{
    CacheModel, LoadEvent, NavigationPolicyDecision, NetworkSession, PolicyDecision,
    PolicyDecisionType, ResponsePolicyDecision, Settings, URISchemeRequest, URISchemeResponse,
    UserContentManager, WebContext, WebProcessTerminationReason, WebView, javascriptcore, soup,
};
use webkit6::{gio, glib, prelude::*};

const APPLICATION_ID: &str = "org.fomalhautdm.Fomalhaut";
const BRIDGE_NAME: &str = "fomalhaut";
const PROTOTYPE_URI: &str = "fomalhaut://theme/";
const NOT_FOUND_BODY: &[u8] = b"The requested prototype resource does not exist.\n";
const METHOD_NOT_ALLOWED_BODY: &[u8] = b"The prototype resource scheme only accepts GET.\n";

/// Runs the native host until its GTK application exits.
pub fn run() -> glib::ExitCode {
    let application = gtk::Application::builder()
        .application_id(APPLICATION_ID)
        .build();
    application.connect_activate(activate);
    application.run()
}

fn activate(application: &gtk::Application) {
    match build_window(application) {
        Ok(window) => {
            window.fullscreen();
            window.present();
        }
        Err(error) => {
            eprintln!("Fomalhaut WebKitGTK host failed to initialize: {error}");
            application.quit();
        }
    }
}

fn build_window(application: &gtk::Application) -> Result<gtk::ApplicationWindow, HostError> {
    let context = WebContext::new();
    context.set_automation_allowed(false);
    context.set_cache_model(CacheModel::DocumentViewer);
    context.set_spell_checking_enabled(false);
    register_scheme(&context)?;

    let content_manager = UserContentManager::new();
    connect_bridge(&content_manager)?;

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
        .default_content_security_policy(PROTOTYPE_CSP)
        .build();
    connect_web_view_policy(&web_view, application);

    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title("Fomalhaut")
        .default_width(1280)
        .default_height(720)
        .child(&web_view)
        .build();
    window.connect_close_request(|_| {
        eprintln!("Fomalhaut host window is closing");
        glib::Propagation::Proceed
    });

    web_view.load_uri(PROTOTYPE_URI);
    Ok(window)
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
                "Fomalhaut served an allowlisted prototype resource ({})",
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
        .expect("embedded prototype resource lengths always fit within signed 64-bit integers");
    let response = URISchemeResponse::new(&stream, length);
    response.set_content_type(content_type);
    response.set_status(status, Some(reason));
    let headers = soup::MessageHeaders::new(soup::MessageHeadersType::Response);
    for (name, value) in PROTOTYPE_HEADERS {
        headers.append(name, value);
    }
    response.set_http_headers(headers);
    request.finish_with_response(&response);
}

fn connect_bridge(content_manager: &UserContentManager) -> Result<(), HostError> {
    content_manager.connect_script_message_with_reply_received(
        Some(BRIDGE_NAME),
        |_, value, reply| {
            eprintln!("Fomalhaut prototype bridge received a script message");
            let Some(input) = value.to_json(0) else {
                eprintln!("Fomalhaut prototype bridge rejected a non-JSON script value");
                reply.return_error_message("the bridge requires a JSON-compatible message");
                return true;
            };

            match handle_prototype_request(input.as_bytes()) {
                PrototypeReply::Json(response) => {
                    let Some(context) = value.context() else {
                        reply
                            .return_error_message("the bridge could not access the script context");
                        return true;
                    };
                    let response = javascriptcore::Value::from_json(&context, &response);
                    reply.return_value(&response);
                }
                PrototypeReply::Rejected(message) => reply.return_error_message(message),
            }
            true
        },
    );

    if !content_manager.register_script_message_handler_with_reply(BRIDGE_NAME, None) {
        return Err(HostError::BridgeRegistration);
    }
    Ok(())
}

fn connect_web_view_policy(web_view: &WebView, application: &gtk::Application) {
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

    let application = application.downgrade();
    web_view.connect_web_process_terminated(move |_, reason| {
        report_web_process_termination(reason);
        if let Some(application) = application.upgrade() {
            application.quit();
        }
    });

    web_view.connect_load_failed(|_, _, _, _| {
        eprintln!("Fomalhaut WebView failed to load an allowed resource");
        true
    });
    web_view.connect_load_changed(|web_view, event| match event {
        LoadEvent::Started => {
            eprintln!("Fomalhaut invalidated the previous page context before loading")
        }
        LoadEvent::Redirected => eprintln!("Fomalhaut observed a WebView redirect"),
        LoadEvent::Committed => eprintln!("Fomalhaut committed an allowlisted page load"),
        LoadEvent::Finished => deliver_prototype_event(web_view),
        _ => eprintln!("Fomalhaut observed an unknown WebView load transition"),
    });
}

fn deliver_prototype_event(web_view: &WebView) {
    match prototype_event_dispatch_script() {
        Ok(script) => {
            web_view.evaluate_javascript(&script, None, None, None::<&gio::Cancellable>, |result| {
                match result {
                    Ok(_) => eprintln!("Fomalhaut prototype delivered a frontend protocol event"),
                    Err(_) => eprintln!("Fomalhaut prototype event delivery failed"),
                }
            })
        }
        Err(error) => eprintln!("Fomalhaut could not construct its prototype event: {error}"),
    }
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
    MissingSecurityManager,
    BridgeRegistration,
}

impl fmt::Display for HostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingSecurityManager => "WebKit did not provide a security manager",
            Self::BridgeRegistration => "the JavaScript message handler could not be registered",
        })
    }
}

impl Error for HostError {}
