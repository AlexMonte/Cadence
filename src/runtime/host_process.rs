//! Standalone runtime-host process entrypoint backed by an embedded WebView.

use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::thread;

#[cfg(target_os = "macos")]
#[link(name = "objc2_exception_helper_0_1", kind = "static")]
unsafe extern "C" {}

use crossbeam_channel::{Receiver, Sender, unbounded};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use url::Url;
use wry::{WebView, WebViewBuilder};

use crate::runtime::protocol::{RuntimeEvent, RuntimeIntent};

const HOST_PAGE_PATH: &str = "runtime/web/index.html";

enum HostUserEvent {
    Intent(RuntimeIntent),
    Shutdown,
}

pub fn run_runtime_host_mode_or_exit() -> ! {
    if let Err(err) = run_runtime_host_mode() {
        eprintln!("{err}");
        std::process::exit(1);
    }
    unreachable!("runtime host mode should not return");
}

fn run_runtime_host_mode() -> Result<(), String> {
    let host_page = resolve_host_page_path()?;
    let host_url = Url::from_file_path(&host_page).map_err(|_| {
        format!(
            "Failed to convert runtime host page path into file URL: {}",
            host_page.display()
        )
    })?;

    let (event_tx, event_rx) = unbounded::<RuntimeEvent>();
    let _stdout_thread = spawn_stdout_writer(event_rx);

    let event_loop = EventLoopBuilder::<HostUserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    spawn_stdin_reader(proxy, event_tx.clone());

    let window = WindowBuilder::new()
        .with_title("GrooveAtlas Runtime Host")
        .with_visible(false)
        .build(&event_loop)
        .map_err(|err| format!("Failed to create runtime host window: {err}"))?;

    let ipc_tx = event_tx.clone();
    let mut webview = WebViewBuilder::new()
        .with_url(host_url.as_str())
        .with_ipc_handler(move |request| {
            handle_ipc_message(&ipc_tx, request.body().clone());
        })
        .build(&window)
        .map_err(|err| format!("Failed to build runtime webview: {err}"))?;

    event_loop.run(move |event, _event_loop_target, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(HostUserEvent::Intent(intent)) => {
                forward_intent_to_webview(&mut webview, &event_tx, intent)
            }
            Event::UserEvent(HostUserEvent::Shutdown) => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

fn resolve_host_page_path() -> Result<PathBuf, String> {
    let mut roots: Vec<PathBuf> = Vec::new();

    if let Ok(root) = std::env::var("GROOVEATLAS_RUNTIME_ROOT") {
        roots.push(PathBuf::from(root));
    }

    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }

    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));

    for root in roots {
        let candidate = root.join(HOST_PAGE_PATH);
        if candidate.is_file() {
            return candidate.canonicalize().map_err(|err| {
                format!(
                    "Failed to canonicalize runtime host page path {}: {err}",
                    candidate.display()
                )
            });
        }
    }

    Err(format!(
        "Runtime host page not found. Expected {} under one of: cwd, GROOVEATLAS_RUNTIME_ROOT, or CARGO_MANIFEST_DIR",
        HOST_PAGE_PATH
    ))
}

fn spawn_stdin_reader(proxy: EventLoopProxy<HostUserEvent>, event_tx: Sender<RuntimeEvent>) {
    thread::spawn(move || {
        let stdin = std::io::stdin();
        let reader = BufReader::new(stdin.lock());

        for line_result in reader.lines() {
            let Ok(line) = line_result else {
                let _ = event_tx.send(RuntimeEvent::error(
                    0,
                    "Failed to read runtime host stdin".to_string(),
                ));
                continue;
            };

            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<RuntimeIntent>(&line) {
                Ok(intent) => {
                    if proxy.send_event(HostUserEvent::Intent(intent)).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let _ = event_tx.send(RuntimeEvent::error(
                        0,
                        format!("Failed to parse runtime host command '{line}': {err}"),
                    ));
                }
            }
        }

        let _ = proxy.send_event(HostUserEvent::Shutdown);
    });
}

fn spawn_stdout_writer(event_rx: Receiver<RuntimeEvent>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let stdout = std::io::stdout();
        let mut writer = BufWriter::new(stdout.lock());

        for event in event_rx.iter() {
            match serde_json::to_string(&event) {
                Ok(line) => {
                    if writeln!(writer, "{line}")
                        .and_then(|_| writer.flush())
                        .is_err()
                    {
                        break;
                    }
                }
                Err(err) => {
                    let fallback = RuntimeEvent::error(
                        0,
                        format!("Failed to serialize runtime event for stdout: {err}"),
                    );
                    if let Ok(line) = serde_json::to_string(&fallback) {
                        let _ = writeln!(writer, "{line}");
                        let _ = writer.flush();
                    }
                }
            }
        }
    })
}

fn forward_intent_to_webview(
    webview: &mut WebView,
    event_tx: &Sender<RuntimeEvent>,
    intent: RuntimeIntent,
) {
    let json = match serde_json::to_string(&intent) {
        Ok(json) => json,
        Err(err) => {
            let _ = event_tx.send(RuntimeEvent::error(
                0,
                format!("Failed to serialize runtime command for webview: {err}"),
            ));
            return;
        }
    };

    let script = format!("window.__grooveatlas_handle_intent({json});");
    if let Err(err) = webview.evaluate_script(&script) {
        let _ = event_tx.send(RuntimeEvent::error(
            0,
            format!("Failed to evaluate runtime host command script: {err}"),
        ));
    }
}

fn handle_ipc_message(event_tx: &Sender<RuntimeEvent>, payload: String) {
    if payload.trim().is_empty() {
        return;
    }

    match serde_json::from_str::<RuntimeEvent>(&payload) {
        Ok(event) => {
            let _ = event_tx.send(event);
        }
        Err(err) => {
            let _ = event_tx.send(RuntimeEvent::error(
                0,
                format!("Failed to parse runtime webview event payload '{payload}': {err}"),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_host_page_path_finds_packaged_runtime_page() {
        let path = resolve_host_page_path().expect("runtime host page should resolve");
        assert!(path.is_file());
        assert!(path.ends_with(HOST_PAGE_PATH));
    }
}
