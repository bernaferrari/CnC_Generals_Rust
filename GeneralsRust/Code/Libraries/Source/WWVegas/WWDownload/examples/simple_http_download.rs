//! Simple HTTP download example

use std::sync::Arc;
use wwdownload::{ConsoleDownloadListener, DownloadManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    wwdownload::init();

    let mut manager = DownloadManager::new()?;
    let listener = Arc::new(ConsoleDownloadListener);

    // Example: Download a small test file
    let url = "https://httpbin.org/bytes/1024"; // 1KB test file
    let local_path = "/tmp/test_download.bin";

    println!("Starting HTTP download from {} to {}", url, local_path);

    let bytes_downloaded = manager
        .download_http(url, local_path, Some(listener))
        .await?;

    println!("Successfully downloaded {} bytes", bytes_downloaded);
    Ok(())
}
