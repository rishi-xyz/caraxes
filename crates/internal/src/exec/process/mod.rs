//! Process capability

use std::{process::Stdio, time::Duration};

use crate::error::{
    Error::{self, Cancelled, ProcessTimeout, SpawnError},
    Result,
};

use tokio::process::Command;

use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

use super::ExecutionRuntime;

// upper bound on captured output per stream so a runaway command can't exhaust mmeory, anything past this is dropped and marked truncated.
// CONFIG! Need to be configurble through file and settings
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// A process to run, described in provider-neutral terms.
#[derive(Debug, Clone, Default)]
pub struct CommandSpec {
    program: String,
    args: Vec<String>,
    cwd: Option<String>,
    env: Vec<(String, String)>,
    stdin: Option<String>,
    timeout: Option<Duration>,
}

impl CommandSpec {
    pub fn new(program: impl Into<String>) -> Self {
        CommandSpec {
            program: program.into(),
            ..CommandSpec::default()
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn cwd(mut self, dir: impl Into<String>) -> Self {
        self.cwd = Some(dir.into());
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn stdin(mut self, input: impl Into<String>) -> Self {
        self.stdin = Some(input.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

/// the observable result of a process that ran to completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    /// the exit code will be none if exitred by a signal
    pub exit_code: Option<i32>,
    pub success: bool,
}

impl ExecutionRuntime {
    /// Run a command to completion, capturing its output
    ///
    /// returns timeout or cancelled if command had to be killed
    pub async fn run(&self, spec: CommandSpec, cancel: CancellationToken) -> Result<CommandOutput> {
        // boundary check
        let cwd = match &spec.cwd {
            Some(dir) => Some(self.workspace().resolve(dir)?),
            None => Some(self.workspace().root().to_path_buf()),
        };

        let mut command = Command::new(&spec.program);

        command
            .args(&spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true); // if the command is drop the child is also dropped rather than left running

        if let Some(cwd) = &cwd {
            command.current_dir(cwd);
        }

        for (key, value) in &spec.env {
            command.env(key, value);
        }

        let mut child = command.spawn().map_err(|src| SpawnError {
            program: spec.program.clone(),
            source: src,
        })?;

        match spec.stdin.clone() {
            Some(input) => {
                if let Some(mut stdin) = child.stdin.take() {
                    tokio::spawn(async move {
                        use tokio::io::AsyncWriteExt;
                        let _ = stdin.write_all(input.as_bytes()).await;
                    });
                }
            }
            None => {
                drop(child.stdin.take());
            }
        }

        let out_task = tokio::spawn(read_capped(child.stdout.take()));
        let err_task = tokio::spawn(read_capped(child.stderr.take()));

        let outcome = match spec.timeout {
            Some(deadline) => {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => Outcome::Cancelled,
                    _ = tokio::time::sleep(deadline) => Outcome::TimeOut(deadline),
                    status = child.wait() => Outcome::Finished(status.map_err(Error::Io)?),
                }
            }
            None => {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => Outcome::Cancelled,
                    status = child.wait() => Outcome::Finished(status.map_err(Error::Io)?),
                }
            }
        };

        match outcome {
            Outcome::Finished(status) => Ok(CommandOutput {
                stdout: join_output(out_task).await,
                stderr: join_output(err_task).await,
                exit_code: status.code(),
                success: status.success(),
            }),
            Outcome::Cancelled => {
                kill(&mut child).await;
                let _ = out_task.await;
                let _ = err_task.await;
                Err(Cancelled)
            }
            Outcome::TimeOut(deadline) => {
                kill(&mut child).await;
                let _ = out_task.await;
                let _ = err_task.await;
                Err(ProcessTimeout(deadline))
            }
        }
    }
}

/// Read a file to EOF or max out put bytes
async fn read_capped<R>(reader: Option<R>) -> std::io::Result<String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(reader) = reader else {
        return Ok(String::new());
    };

    let mut buf = Vec::new();

    reader
        .take((MAX_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut buf)
        .await?;

    if buf.len() > MAX_OUTPUT_BYTES {
        buf.truncate(MAX_OUTPUT_BYTES);
        let mut text = String::from_utf8_lossy(&buf).into_owned();
        text.push_str("\n ... [output truncated]");
        Ok(text)
    } else {
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }
}

enum Outcome {
    Finished(std::process::ExitStatus),
    Cancelled,
    TimeOut(Duration),
}

async fn kill(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

async fn join_output(handle: tokio::task::JoinHandle<std::io::Result<String>>) -> String {
    match handle.await {
        Ok(Ok(text)) => text,
        _ => String::new(),
    }
}
