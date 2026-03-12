//! DownloadManager - async file downloader matching C++ DownloadManager behavior.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use futures_util::StreamExt;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::error::{NetworkError, NetworkResult};

pub const DOWNLOADEVENT_NOSUCHSERVER: i32 = 1;
pub const DOWNLOADEVENT_COULDNOTCONNECT: i32 = 2;
pub const DOWNLOADEVENT_LOGINFAILED: i32 = 3;
pub const DOWNLOADEVENT_NOSUCHFILE: i32 = 4;
pub const DOWNLOADEVENT_LOCALFILEOPENFAILED: i32 = 5;
pub const DOWNLOADEVENT_TCPERROR: i32 = 6;
pub const DOWNLOADEVENT_DISCONNECTERROR: i32 = 7;

pub const DOWNLOADSTATUS_NONE: i32 = 0;
pub const DOWNLOADSTATUS_GO: i32 = 1;
pub const DOWNLOADSTATUS_CONNECTING: i32 = 2;
pub const DOWNLOADSTATUS_LOGGINGIN: i32 = 3;
pub const DOWNLOADSTATUS_FINDINGFILE: i32 = 4;
pub const DOWNLOADSTATUS_QUERYINGRESUME: i32 = 5;
pub const DOWNLOADSTATUS_DOWNLOADING: i32 = 6;
pub const DOWNLOADSTATUS_DISCONNECTING: i32 = 7;
pub const DOWNLOADSTATUS_FINISHING: i32 = 8;
pub const DOWNLOADSTATUS_DONE: i32 = 0;

#[derive(Debug, Clone)]
pub struct QueuedDownload {
    pub server: String,
    pub user_name: String,
    pub password: String,
    pub file: String,
    pub local_file: String,
    pub reg_key: String,
    pub try_resume: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    pub bytes_read: i64,
    pub total_size: i64,
    pub time_taken: i64,
    pub time_left: i64,
}

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    FileStarted(String),
    StatusUpdate(i32),
    Progress(DownloadProgress),
    Error(i32),
    End,
}

struct ActiveDownload {
    start: Instant,
    total_size: i64,
    downloaded: i64,
}

impl Default for ActiveDownload {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            total_size: 0,
            downloaded: 0,
        }
    }
}

pub struct DownloadManager {
    queued: VecDeque<QueuedDownload>,
    events_tx: mpsc::UnboundedSender<DownloadEvent>,
    events_rx: mpsc::UnboundedReceiver<DownloadEvent>,
    status_key: String,
    error_key: String,
    was_error: bool,
    saw_end: bool,
    last_local_file: String,
    current_file: String,
    last_progress: Option<DownloadProgress>,
    active: Option<ActiveDownload>,
}

impl DownloadManager {
    pub fn new() -> Self {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        Self {
            queued: VecDeque::new(),
            events_tx,
            events_rx,
            status_key: "FTP:StatusIdle".to_string(),
            error_key: String::new(),
            was_error: false,
            saw_end: false,
            last_local_file: String::new(),
            current_file: String::new(),
            last_progress: None,
            active: None,
        }
    }

    pub fn status_key(&self) -> &str {
        &self.status_key
    }

    pub fn error_key(&self) -> &str {
        &self.error_key
    }

    pub fn last_local_file(&self) -> &str {
        &self.last_local_file
    }

    pub fn current_file(&self) -> &str {
        &self.current_file
    }

    pub fn last_progress(&self) -> Option<DownloadProgress> {
        self.last_progress
    }

    pub fn is_done(&self) -> bool {
        self.saw_end || self.was_error
    }

    pub fn is_ok(&self) -> bool {
        self.saw_end
    }

    pub fn was_error(&self) -> bool {
        self.was_error
    }

    pub fn queue_file_for_download(&mut self, download: QueuedDownload) {
        self.queued.push_back(download);
    }

    pub fn is_file_queued_for_download(&self) -> bool {
        !self.queued.is_empty()
    }

    pub fn download_next_queued_file(&mut self) -> NetworkResult<()> {
        let Some(download) = self.queued.pop_front() else {
            return Ok(());
        };
        self.was_error = false;
        self.saw_end = false;
        self.download_file(download)
    }

    pub fn download_file(&mut self, download: QueuedDownload) -> NetworkResult<()> {
        if self.active.is_some() {
            return Err(NetworkError::generic("Download already active"));
        }
        self.last_local_file = download.local_file.clone();
        self.events_tx
            .send(DownloadEvent::FileStarted(download.file.clone()))
            .ok();
        self.events_tx
            .send(DownloadEvent::StatusUpdate(DOWNLOADSTATUS_CONNECTING))
            .ok();
        self.active = Some(ActiveDownload {
            start: Instant::now(),
            total_size: 0,
            downloaded: 0,
        });

        let events_tx = self.events_tx.clone();
        tokio::spawn(async move {
            if let Err(error) = run_download(download, events_tx.clone()).await {
                let _ = events_tx.send(DownloadEvent::Error(error));
            }
        });
        Ok(())
    }

    pub fn update(&mut self) -> Vec<DownloadEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.events_rx.try_recv() {
            self.apply_event(&event);
            events.push(event);
        }
        events
    }

    fn apply_event(&mut self, event: &DownloadEvent) {
        match event {
            DownloadEvent::StatusUpdate(status) => {
                self.status_key = status_key(*status).to_string();
            }
            DownloadEvent::FileStarted(file) => {
                self.current_file = file.clone();
            }
            DownloadEvent::Progress(progress) => {
                self.last_progress = Some(*progress);
            }
            DownloadEvent::Error(code) => {
                self.error_key = error_key(*code).to_string();
                self.was_error = true;
                self.active = None;
            }
            DownloadEvent::End => {
                self.saw_end = true;
                self.active = None;
            }
        }
    }
}

static THE_DOWNLOAD_MANAGER: OnceLock<Mutex<Option<DownloadManager>>> = OnceLock::new();

pub fn download_manager() -> &'static Mutex<Option<DownloadManager>> {
    THE_DOWNLOAD_MANAGER.get_or_init(|| Mutex::new(None))
}

pub fn set_download_manager(manager: Option<DownloadManager>) {
    let mut guard = download_manager()
        .lock()
        .expect("DownloadManager mutex poisoned");
    *guard = manager;
}

fn status_key(status: i32) -> &'static str {
    match status {
        DOWNLOADSTATUS_CONNECTING => "FTP:StatusConnecting",
        DOWNLOADSTATUS_LOGGINGIN => "FTP:StatusLoggingIn",
        DOWNLOADSTATUS_FINDINGFILE => "FTP:StatusFindingFile",
        DOWNLOADSTATUS_QUERYINGRESUME => "FTP:StatusQueryingResume",
        DOWNLOADSTATUS_DOWNLOADING => "FTP:StatusDownloading",
        DOWNLOADSTATUS_DISCONNECTING => "FTP:StatusDisconnecting",
        DOWNLOADSTATUS_FINISHING => "FTP:StatusFinishing",
        DOWNLOADSTATUS_DONE => "FTP:StatusDone",
        _ => "FTP:StatusNone",
    }
}

fn error_key(error: i32) -> &'static str {
    match error {
        DOWNLOADEVENT_NOSUCHSERVER => "FTP:NoSuchServer",
        DOWNLOADEVENT_COULDNOTCONNECT => "FTP:CouldNotConnect",
        DOWNLOADEVENT_LOGINFAILED => "FTP:LoginFailed",
        DOWNLOADEVENT_NOSUCHFILE => "FTP:NoSuchFile",
        DOWNLOADEVENT_LOCALFILEOPENFAILED => "FTP:LocalFileOpenFailed",
        DOWNLOADEVENT_TCPERROR => "FTP:TCPError",
        DOWNLOADEVENT_DISCONNECTERROR => "FTP:DisconnectError",
        _ => "FTP:UnknownError",
    }
}

fn build_url(server: &str, file: &str) -> Option<String> {
    if file.contains("://") {
        return Some(file.to_string());
    }
    if server.contains("://") {
        let base = server.trim_end_matches('/');
        let path = file.trim_start_matches('/');
        return Some(format!("{}/{}", base, path));
    }
    if server.is_empty() {
        return None;
    }
    let path = file.trim_start_matches('/');
    Some(format!("http://{}/{}", server, path))
}

async fn run_download(
    download: QueuedDownload,
    events_tx: mpsc::UnboundedSender<DownloadEvent>,
) -> Result<(), i32> {
    let url = build_url(download.server.as_str(), download.file.as_str())
        .ok_or(DOWNLOADEVENT_NOSUCHSERVER)?;

    if download.user_name.trim().is_empty() {
        let _ = events_tx.send(DownloadEvent::StatusUpdate(DOWNLOADSTATUS_FINDINGFILE));
    } else {
        let _ = events_tx.send(DownloadEvent::StatusUpdate(DOWNLOADSTATUS_LOGGINGIN));
    }

    let mut request = reqwest::Client::new().get(&url);
    if !download.user_name.trim().is_empty() {
        request = request.basic_auth(download.user_name.clone(), Some(download.password.clone()));
    }

    let mut resume_from = 0u64;
    if download.try_resume {
        let local_path = Path::new(download.local_file.as_str());
        if let Ok(metadata) = std::fs::metadata(local_path) {
            resume_from = metadata.len();
            if resume_from > 0 {
                let _ = events_tx.send(DownloadEvent::StatusUpdate(DOWNLOADSTATUS_QUERYINGRESUME));
                request = request.header(reqwest::header::RANGE, format!("bytes={}-", resume_from));
            }
        }
    }

    let response = request
        .send()
        .await
        .map_err(|err| map_reqwest_error(&err))?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(DOWNLOADEVENT_LOGINFAILED);
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(DOWNLOADEVENT_NOSUCHFILE);
    }
    if !status.is_success() {
        return Err(DOWNLOADEVENT_TCPERROR);
    }

    let total_size = response
        .content_length()
        .map(|len| len.saturating_add(resume_from))
        .unwrap_or(0);
    let _ = events_tx.send(DownloadEvent::StatusUpdate(DOWNLOADSTATUS_DOWNLOADING));

    let mut file = open_output_file(download.local_file.as_str(), resume_from > 0)
        .await
        .map_err(|_| DOWNLOADEVENT_LOCALFILEOPENFAILED)?;

    if resume_from > 0 && status == reqwest::StatusCode::OK {
        file = open_output_file(download.local_file.as_str(), false)
            .await
            .map_err(|_| DOWNLOADEVENT_LOCALFILEOPENFAILED)?;
        resume_from = 0;
    }

    let start = Instant::now();
    let mut downloaded = resume_from;
    let mut stream = response.bytes_stream();

    while let Some(next) = stream.next().await {
        let chunk = next.map_err(|_| DOWNLOADEVENT_TCPERROR)?;
        file.write_all(&chunk)
            .await
            .map_err(|_| DOWNLOADEVENT_LOCALFILEOPENFAILED)?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        let time_taken = start.elapsed().as_secs().max(1);
        let time_left = if total_size > 0 && downloaded > 0 {
            let remaining = total_size.saturating_sub(downloaded);
            let bytes_per_sec = downloaded / time_taken;
            if bytes_per_sec == 0 {
                0
            } else {
                remaining / bytes_per_sec
            }
        } else {
            0
        };
        let _ = events_tx.send(DownloadEvent::Progress(DownloadProgress {
            bytes_read: downloaded as i64,
            total_size: total_size as i64,
            time_taken: time_taken as i64,
            time_left: time_left as i64,
        }));
    }

    let _ = events_tx.send(DownloadEvent::StatusUpdate(DOWNLOADSTATUS_FINISHING));
    let _ = file.flush().await;
    let _ = events_tx.send(DownloadEvent::StatusUpdate(DOWNLOADSTATUS_DONE));
    let _ = events_tx.send(DownloadEvent::End);
    Ok(())
}

async fn open_output_file(path: &str, append: bool) -> std::io::Result<tokio::fs::File> {
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(!append)
        .append(append)
        .open(path)
        .await
}

fn map_reqwest_error(error: &reqwest::Error) -> i32 {
    if error.is_connect() {
        return DOWNLOADEVENT_COULDNOTCONNECT;
    }
    if error.is_timeout() {
        return DOWNLOADEVENT_TCPERROR;
    }
    if error.is_request() {
        return DOWNLOADEVENT_NOSUCHSERVER;
    }
    DOWNLOADEVENT_TCPERROR
}
