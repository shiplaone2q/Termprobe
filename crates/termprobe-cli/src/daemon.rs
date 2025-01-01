//! cli daemon host: local socket listener, idle watchdog, monitor streaming,
//! and process state files around the reusable in-process engine.

use std::io::{BufReader, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{ListenerNonblockingMode, Stream};

use termprobe::engine::Engine;
use termprobe::logger::Logger;
use termprobe::Operation;

use crate::protocol::{Request, Response};
use crate::{config, ipc, monitor};

const MAX_PENDING_REQUESTS: usize = 64;
const MAX_PENDING_WAITS: usize = 64;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

struct Host {
    engine: Arc<Engine>,
    session: String,
    logging: bool,
    status: Mutex<Response>,
    last_activity: Arc<Mutex<Instant>>,
    stopping: AtomicBool,
    waiters: Arc<AtomicUsize>,
}

struct ConnectionPermit(Arc<AtomicUsize>);

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn try_acquire_permit(counter: &AtomicUsize, limit: usize) -> bool {
    let mut count = counter.load(Ordering::Acquire);
    loop {
        if count >= limit {
            return false;
        }
        match counter.compare_exchange_weak(count, count + 1, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return true,
            Err(current) => count = current,
        }
    }
}

pub fn run(session_name: String, verbose: bool) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        // A caller may itself have disabled Ctrl+C. Do not pass that process
        // attribute on to the shells we create in ConPTY.
        unsafe { windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 0) };
    }
    config::ensure_home()?;
    sweep_recordings(&session_name);
    let socket = config::socket_name(&session_name);
    let listener = ipc::listen(&socket)?;
    listener.set_nonblocking(ListenerNonblockingMode::Accept)?;
    std::fs::write(
        config::pid_file(&session_name),
        std::process::id().to_string(),
    )
    .ok();

    let logger = if verbose {
        match Logger::to_file(&config::log_file(&session_name)) {
            Ok(logger) => Arc::new(logger),
            Err(_) => Arc::new(Logger::disabled()),
        }
    } else {
        Arc::new(Logger::disabled())
    };
    let logging = logger.enabled();
    let engine = Arc::new(Engine::new(
        session_name.clone(),
        logger,
        config::recording_file(&session_name),
    ));
    engine.log_event(&format!(
        "daemon start session={session_name} pid={}",
        std::process::id()
    ));
    let host = Arc::new(Host {
        status: Mutex::new(status_response(&engine)),
        engine,
        session: session_name.clone(),
        logging,
        last_activity: Arc::new(Mutex::new(Instant::now())),
        stopping: AtomicBool::new(false),
        waiters: Arc::new(AtomicUsize::new(0)),
    });
    let (shutdown, shutdown_requests) = mpsc::channel();
    spawn_idle_watchdog(Arc::clone(&host), shutdown.clone());
    let (operations, requests) = mpsc::sync_channel(MAX_PENDING_REQUESTS);
    spawn_operation_worker(requests, Arc::clone(&host));
    let readers = Arc::new(AtomicUsize::new(0));

    let mut listener_error = None;
    let closing_client = loop {
        if let Ok(client) = shutdown_requests.try_recv() {
            break client;
        }
        match listener.accept() {
            Ok(conn) => {
                if !try_acquire_permit(&readers, MAX_PENDING_REQUESTS) {
                    continue;
                }
                let permit = ConnectionPermit(Arc::clone(&readers));
                let host = Arc::clone(&host);
                let operations = operations.clone();
                let shutdown = shutdown.clone();
                std::thread::spawn(move || {
                    let _permit = permit;
                    handle_connection(conn, &host, &operations, &shutdown);
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::Interrupted
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::ConnectionReset
                ) => {}
            Err(error) => {
                listener_error = Some(error);
                break None;
            }
        }
    };
    host.stopping.store(true, Ordering::Release);
    drop(operations);
    let response = shutdown_engine(Arc::clone(&host));
    if let Some(mut conn) = closing_client {
        let _ = ipc::write_response(&mut conn, &response);
        ipc::drain_peer(conn, Duration::from_millis(config::SHUTDOWN_DRAIN_MS));
    }
    cleanup(&session_name);
    listener_error.map_or(Ok(()), |error| Err(error.into()))
}

fn handle_connection(
    conn: Stream,
    host: &Arc<Host>,
    operations: &mpsc::SyncSender<(Request, Stream)>,
    shutdown: &mpsc::Sender<Option<Stream>>,
) {
    let mut reader = BufReader::new(conn);
    let req = match ipc::read_request_with_timeout(&mut reader, ipc::REQUEST_TIMEOUT) {
        Ok(request) => request,
        Err(_) => return,
    };
    *host.last_activity.lock().unwrap() = Instant::now();
    if req.is_close() || req.is_shutdown() {
        if !host.stopping.swap(true, Ordering::AcqRel) {
            let _ = shutdown.send(Some(reader.into_inner()));
