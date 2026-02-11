//! Native runtime-host process launcher and IPC transport plumbing.

use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;

use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::runtime::RUNTIME_HOST_FLAG;
use crate::runtime::protocol::{RuntimeEvent, RuntimeIntent};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<RuntimeHostConfig>()
        .add_systems(Startup, setup_runtime_host);
}

#[derive(Resource, Debug, Clone)]
pub struct RuntimeHostConfig {
    pub host_flag: String,
    pub executable: Option<PathBuf>,
    pub working_dir: PathBuf,
    pub enabled: bool,
}

impl Default for RuntimeHostConfig {
    fn default() -> Self {
        Self {
            host_flag: RUNTIME_HOST_FLAG.to_string(),
            executable: None,
            working_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            enabled: true,
        }
    }
}

#[derive(Resource, Clone)]
pub struct RuntimeHost {
    command_tx: Sender<RuntimeIntent>,
    event_rx: Receiver<RuntimeEvent>,
}

impl RuntimeHost {
    pub fn try_send(&self, intent: RuntimeIntent) -> Result<(), String> {
        self.command_tx
            .try_send(intent)
            .map_err(|err| err.to_string())
    }

    pub fn try_drain_events(&self) -> Vec<RuntimeEvent> {
        self.event_rx.try_iter().collect()
    }
}

fn setup_runtime_host(mut commands: Commands, config: Res<RuntimeHostConfig>) {
    if !config.enabled {
        return;
    }

    let (command_tx, command_rx) = unbounded::<RuntimeIntent>();
    let (event_tx, event_rx) = unbounded::<RuntimeEvent>();
    let config = config.clone();

    thread::spawn(move || run_runtime_host(config, command_rx, event_tx));

    commands.insert_resource(RuntimeHost {
        command_tx,
        event_rx,
    });
}

fn run_runtime_host(
    config: RuntimeHostConfig,
    command_rx: Receiver<RuntimeIntent>,
    event_tx: Sender<RuntimeEvent>,
) {
    let executable = match resolve_executable_path(&config) {
        Ok(path) => path,
        Err(err) => {
            let _ = event_tx.send(RuntimeEvent::error(0, err));
            return;
        }
    };

    let mut child = match Command::new(&executable)
        .arg(&config.host_flag)
        .current_dir(&config.working_dir)
        .env("GROOVEATLAS_RUNTIME_ROOT", &config.working_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            let _ = event_tx.send(RuntimeEvent::error(
                0,
                format!(
                    "Failed to launch runtime host process ({} {}): {err}",
                    executable.display(),
                    config.host_flag
                ),
            ));
            return;
        }
    };

    let Some(stdout) = child.stdout.take() else {
        let _ = event_tx.send(RuntimeEvent::error(
            0,
            "Runtime host missing stdout pipe".to_string(),
        ));
        return;
    };
    let Some(stderr) = child.stderr.take() else {
        let _ = event_tx.send(RuntimeEvent::error(
            0,
            "Runtime host missing stderr pipe".to_string(),
        ));
        return;
    };
    let Some(stdin) = child.stdin.take() else {
        let _ = event_tx.send(RuntimeEvent::error(
            0,
            "Runtime host missing stdin pipe".to_string(),
        ));
        return;
    };

    let event_tx_stdout = event_tx.clone();
    let stdout_thread = thread::spawn(move || read_host_stdout(stdout, event_tx_stdout));

    let event_tx_stderr = event_tx.clone();
    let stderr_thread = thread::spawn(move || read_host_stderr(stderr, event_tx_stderr));

    let mut stdin_writer = BufWriter::new(stdin);
    for intent in command_rx.iter() {
        match serde_json::to_string(&intent) {
            Ok(line) => {
                if writeln!(stdin_writer, "{line}")
                    .and_then(|_| stdin_writer.flush())
                    .is_err()
                {
                    let _ = event_tx.send(RuntimeEvent::error(
                        0,
                        "Failed to send command to runtime host".to_string(),
                    ));
                    break;
                }
            }
            Err(err) => {
                let _ = event_tx.send(RuntimeEvent::error(
                    0,
                    format!("Failed to serialize runtime command: {err}"),
                ));
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();
}

fn resolve_executable_path(config: &RuntimeHostConfig) -> Result<PathBuf, String> {
    let executable = if let Some(path) = &config.executable {
        path.clone()
    } else {
        std::env::current_exe().map_err(|err| {
            format!("Failed to resolve current executable for runtime host spawn: {err}")
        })?
    };

    if executable.is_file() {
        Ok(executable)
    } else {
        Err(format!(
            "Runtime host executable not found at {}",
            executable.display()
        ))
    }
}

fn read_host_stdout(stdout: impl std::io::Read, event_tx: Sender<RuntimeEvent>) {
    let reader = BufReader::new(stdout);
    for line_result in reader.lines() {
        let Ok(line) = line_result else {
            let _ = event_tx.send(RuntimeEvent::error(
                0,
                "Failed to read runtime host stdout".to_string(),
            ));
            continue;
        };

        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<RuntimeEvent>(&line) {
            Ok(event) => {
                let _ = event_tx.send(event);
            }
            Err(err) => {
                let _ = event_tx.send(RuntimeEvent::error(
                    0,
                    format!("Failed to parse runtime host message '{line}': {err}"),
                ));
            }
        }
    }
}

fn read_host_stderr(stderr: impl std::io::Read, event_tx: Sender<RuntimeEvent>) {
    let reader = BufReader::new(stderr);
    for line_result in reader.lines() {
        let Ok(line) = line_result else {
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        let _ = event_tx.send(RuntimeEvent::error(
            0,
            format!("Runtime host stderr: {line}"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_executable_path_uses_current_exe_by_default() {
        let config = RuntimeHostConfig::default();
        let path =
            resolve_executable_path(&config).expect("default current executable should resolve");
        assert!(path.is_file());
    }

    #[test]
    fn resolve_executable_path_rejects_directory_override() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let config = RuntimeHostConfig {
            executable: Some(temp.path().to_path_buf()),
            ..RuntimeHostConfig::default()
        };

        let err = resolve_executable_path(&config)
            .expect_err("directory override should be rejected as executable path");
        assert!(err.contains("Runtime host executable not found"));
    }
}
