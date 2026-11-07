use drift_core::Config;
use serde::Deserialize;
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
#[derive(Clone, Default)]
pub struct Runtime(Arc<Inner>);
#[derive(Default)]
struct Inner {
    operation: Mutex<()>,
    session: Mutex<Option<Session>>,
    busy: AtomicBool,
    error: Mutex<Option<String>>,
}
struct Session {
    child: Child,
}
#[derive(Deserialize)]
struct Reply {
    ok: bool,
    error: Option<String>,
}
impl Runtime {
    pub fn initialize() -> Self {
        let runtime = Self::default();
        #[cfg(any(windows, target_os = "linux"))]
        {
            let result = helper().and_then(|path| {
                let mut command = Command::new(path);
                command.arg("--recover");
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    command.creation_flags(0x08000000);
                }
                command.output()
                    .map_err(|e| e.to_string())
                    .and_then(|output| {
                        if output.status.success() {
                            Ok(())
                        } else {
                            Err("Could not restore the previous proxy settings. Restart Drift to retry.".into())
                        }
                    })
            });
            if let Err(error) = result {
                *runtime.0.error.lock().unwrap() = Some(error);
            }
        }
        runtime
    }
    pub fn state(&self) -> (&'static str, String) {
        if self.0.busy.load(Ordering::Relaxed) {
            return ("preparing", String::new());
        }
        let mut guard = self.0.session.lock().unwrap();
        if let Some(session) = guard.as_mut() {
            if session.child.try_wait().ok().flatten().is_none() {
                return ("active", String::new());
            }
            *guard = None;
            *self.0.error.lock().unwrap() =
                Some("The network connection stopped. Turn access on to reconnect.".into());
        }
        if let Some(error) = self.0.error.lock().unwrap().as_ref() {
            ("error", error.clone())
        } else {
            ("off", String::new())
        }
    }
    pub fn active(&self) -> bool {
        self.state().0 == "active"
    }
    pub fn set(&self, enabled: bool, config: &Config) -> Result<(), String> {
        let _operation = self.0.operation.lock().unwrap();
        self.0.busy.store(true, Ordering::Relaxed);
        struct Busy<'a>(&'a AtomicBool);
        impl Drop for Busy<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Relaxed);
            }
        }
        let _busy = Busy(&self.0.busy);
        let previous = self.0.session.lock().unwrap().take();
        drop(previous);
        *self.0.error.lock().unwrap() = None;
        if !enabled {
            return Ok(());
        }
        let result = Session::start(config);
        match result {
            Ok(session) => {
                *self.0.session.lock().unwrap() = Some(session);
                Ok(())
            }
            Err(error) => {
                *self.0.error.lock().unwrap() = Some(error.clone());
                Err(error)
            }
        }
    }
}
fn helper() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let helper = executable
        .parent()
        .ok_or("Application directory missing")?
        .join(if cfg!(windows) {
            "drift-agent.exe"
        } else {
            "drift-agent"
        });
    if !helper.is_file() {
        return Err("The networking helper is missing from this Drift installation.".into());
    }
    Ok(helper)
}
impl Session {
    fn start(config: &Config) -> Result<Self, String> {
        config.validate().map_err(|e| e.to_string())?;
        let mut command = Command::new(helper()?);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut session = Self {
            child: command.spawn().map_err(|e| e.to_string())?,
        };
        let bytes = serde_json::to_vec(config).map_err(|e| e.to_string())?;
        if bytes.len() > 65536 {
            return Err("The rule list is too large.".into());
        }
        let input = session
            .child
            .stdin
            .as_mut()
            .ok_or("Network control pipe unavailable")?;
        input
            .write_all(&bytes)
            .and_then(|_| input.write_all(b"\n"))
            .and_then(|_| input.flush())
            .map_err(|e| e.to_string())?;
        let output = session
            .child
            .stdout
            .take()
            .ok_or("Network status pipe unavailable")?;
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut response = String::new();
            let result = std::io::Read::take(BufReader::new(output), 8192)
                .read_line(&mut response)
                .map(|_| response);
            let _ = send.send(result);
        });
        let line = receive
            .recv_timeout(Duration::from_secs(90))
            .map_err(|_| "Network setup timed out.".to_owned())?
            .map_err(|e| e.to_string())?;
        let reply: Reply = serde_json::from_str(&line)
            .map_err(|_| "The networking helper stopped before it was ready.".to_owned())?;
        if !reply.ok {
            return Err(reply
                .error
                .unwrap_or_else(|| "Could not enable access.".into()));
        }
        if session
            .child
            .try_wait()
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Err("The networking helper stopped unexpectedly.".into());
        }
        Ok(session)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(mut input) = self.child.stdin.take() {
            let _ = input.write_all(b"stop\n");
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Err(_) => break,
                _ => {}
            }
            if Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
