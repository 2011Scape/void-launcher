//! Shared HTTP client: JSON with retry, pagination, and hashed streaming downloads.

use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use sipper::Straw;
use tokio::io::AsyncWriteExt;

use crate::error::{CoreError, IoContext};
use crate::hash::same_digest;
use crate::progress::Progress;

const PROGRESS_STEP: u64 = 256 * 1024;

#[derive(Clone)]
pub struct Http {
    client: reqwest::Client,
}

impl Http {
    pub fn new(user_agent: &str) -> Result<Self, CoreError> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(60))
            .https_only(true)
            .build()
            .map_err(|e| CoreError::other(format!("building HTTP client: {e}")))?;
        Ok(Self { client })
    }

    /// GET + JSON decode with one retry on connection problems or 5xx.
    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, CoreError> {
        let mut last = None;
        for attempt in 0..2 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            match self.get_json_once(url).await {
                Ok(value) => return Ok(value),
                Err(e) if is_retryable(&e) => {
                    tracing::debug!("GET {url} failed ({e}); retrying");
                    last = Some(e);
                }
                Err(e) => return Err(e),
            }
        }
        Err(last.expect("at least one attempt"))
    }

    async fn get_json_once<T: DeserializeOwned>(&self, url: &str) -> Result<T, CoreError> {
        let response = tokio::time::timeout(Duration::from_secs(30), self.client.get(url).send())
            .await
            .map_err(|_| CoreError::Offline(format!("timed out contacting {url}")))?
            .map_err(|e| CoreError::from_reqwest(url, e))?;
        let status = response.status();
        if !status.is_success() {
            return Err(CoreError::HttpStatus { url: url.to_string(), status: status.as_u16() });
        }
        response.json::<T>().await.map_err(|e| CoreError::BadResponse { url: url.to_string(), reason: e.to_string() })
    }

    /// GET as text with one retry, for small files such as `SHA256SUMS`.
    pub async fn get_text(&self, url: &str) -> Result<String, CoreError> {
        let mut last = None;
        for attempt in 0..2 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            match self.get_text_once(url).await {
                Ok(value) => return Ok(value),
                Err(e) if is_retryable(&e) => {
                    tracing::debug!("GET {url} failed ({e}); retrying");
                    last = Some(e);
                }
                Err(e) => return Err(e),
            }
        }
        Err(last.expect("at least one attempt"))
    }

    async fn get_text_once(&self, url: &str) -> Result<String, CoreError> {
        let response = tokio::time::timeout(Duration::from_secs(30), self.client.get(url).send())
            .await
            .map_err(|_| CoreError::Offline(format!("timed out contacting {url}")))?
            .map_err(|e| CoreError::from_reqwest(url, e))?;
        let status = response.status();
        if !status.is_success() {
            return Err(CoreError::HttpStatus { url: url.to_string(), status: status.as_u16() });
        }
        response.text().await.map_err(|e| CoreError::BadResponse { url: url.to_string(), reason: e.to_string() })
    }

    /// `Content-Length` of a file, following redirects (GitHub release assets
    /// redirect to a CDN). Errors are not fatal for callers: the download
    /// reports its own length.
    pub async fn content_length(&self, url: &str) -> Option<u64> {
        let response = tokio::time::timeout(Duration::from_secs(15), self.client.head(url).send()).await.ok()?.ok()?;
        if !response.status().is_success() {
            return None;
        }
        response.content_length().filter(|n| *n > 0)
    }

    /// Follows `x-next-page` pagination (GitLab-style). `url` must already contain a `?`.
    pub async fn get_json_paginated<T: DeserializeOwned>(&self, url: &str) -> Result<Vec<T>, CoreError> {
        let mut page = 1u32;
        let mut all = Vec::new();
        loop {
            let page_url = format!("{url}&per_page=100&page={page}");
            let response = self.client.get(&page_url).send().await.map_err(|e| CoreError::from_reqwest(&page_url, e))?;
            let status = response.status();
            if !status.is_success() {
                return Err(CoreError::HttpStatus { url: page_url, status: status.as_u16() });
            }
            let next = response.headers().get("x-next-page").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<u32>().ok());
            let mut items: Vec<T> = response.json().await.map_err(|e| CoreError::BadResponse { url: page_url.clone(), reason: e.to_string() })?;
            all.append(&mut items);
            match next {
                Some(n) if n > page && page < 50 => page = n,
                _ => return Ok(all),
            }
        }
    }

    /// Streams `url` into `dest` (the caller passes a `.part` path), hashing as it
    /// goes and reporting progress. Verifies the checksum when one is expected and
    /// deletes the partial file on any failure. Returns the SHA-256 of the file.
    pub fn download(
        &self,
        url: String,
        dest: PathBuf,
        expected_sha256: Option<String>,
        expected_size: Option<u64>,
    ) -> impl Straw<String, Progress, CoreError> + Send + 'static {
        let client = self.client.clone();
        sipper::sipper(move |mut progress: sipper::Sender<Progress>| async move {
            let mut last_err = None;
            for attempt in 0..2 {
                if attempt > 0 {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                match download_once(&client, &url, &dest, expected_size, &mut progress).await {
                    Ok(sha) => {
                        if let Some(expected) = &expected_sha256
                            && !same_digest(expected, &sha)
                        {
                            let _ = tokio::fs::remove_file(&dest).await;
                            return Err(CoreError::ChecksumMismatch { path: dest, expected: expected.clone(), actual: sha });
                        }
                        return Ok(sha);
                    }
                    Err(e) => {
                        let _ = tokio::fs::remove_file(&dest).await;
                        if is_retryable(&e) {
                            tracing::debug!("download of {url} failed ({e}); retrying");
                            last_err = Some(e);
                        } else {
                            return Err(e);
                        }
                    }
                }
            }
            Err(last_err.expect("at least one attempt"))
        })
    }
}

async fn download_once(
    client: &reqwest::Client,
    url: &str,
    dest: &PathBuf,
    expected_size: Option<u64>,
    progress: &mut sipper::Sender<Progress>,
) -> Result<String, CoreError> {
    let response = client.get(url).send().await.map_err(|e| CoreError::from_reqwest(url, e))?;
    let status = response.status();
    if !status.is_success() {
        return Err(CoreError::HttpStatus { url: url.to_string(), status: status.as_u16() });
    }
    let total = response.content_length().or(expected_size);
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await.context(format!("creating {}", parent.display()))?;
    }
    let mut file = tokio::fs::File::create(dest).await.context(format!("creating {}", dest.display()))?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;
    let mut last_report = 0u64;
    progress.send(Progress::Bytes { downloaded: 0, total }).await;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| CoreError::from_reqwest(url, e))?;
        hasher.update(&chunk);
        file.write_all(&chunk).await.context(format!("writing {}", dest.display()))?;
        downloaded += chunk.len() as u64;
        if downloaded - last_report >= PROGRESS_STEP {
            last_report = downloaded;
            progress.send(Progress::Bytes { downloaded, total }).await;
        }
    }
    file.flush().await.context(format!("flushing {}", dest.display()))?;
    file.sync_all().await.context(format!("syncing {}", dest.display()))?;
    progress.send(Progress::Bytes { downloaded, total: total.or(Some(downloaded)) }).await;
    Ok(hex::encode(hasher.finalize()))
}

fn is_retryable(e: &CoreError) -> bool {
    match e {
        CoreError::Offline(_) => true,
        CoreError::HttpStatus { status, .. } => *status >= 500,
        CoreError::Http { source, .. } => source.is_body() || source.is_decode(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sipper::Sipper;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn download_verifies_sha_and_reports_progress() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/file.bin")).respond_with(ResponseTemplate::new(200).set_body_bytes(vec![7u8; 600_000])).mount(&server).await;
        let http = Http { client: reqwest::Client::new() };
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("file.part");
        let expected = crate::hash::sha256_bytes(&vec![7u8; 600_000]);

        let mut seen = Vec::new();
        let result = http
            .download(format!("{}/file.bin", server.uri()), dest.clone(), Some(expected.clone()), None)
            .with(|p| p)
            .run(sipper::Sender::channel(64).0)
            .await;
        // Re-run capturing progress through a channel.
        let (tx, mut rx) = sipper::Sender::channel(64);
        let sha = http.download(format!("{}/file.bin", server.uri()), dest.clone(), Some(expected.clone()), None).run(tx).await.unwrap();
        while let Ok(Some(p)) = tokio::time::timeout(Duration::from_millis(10), rx.next()).await {
            seen.push(p);
        }
        assert!(result.is_ok());
        assert_eq!(sha, expected);
        assert!(std::fs::metadata(&dest).unwrap().len() == 600_000);
        assert!(seen.iter().any(|p| matches!(p, Progress::Bytes { downloaded, .. } if *downloaded == 600_000)));
    }

    #[tokio::test]
    async fn download_rejects_bad_checksum_and_cleans_up() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/file.bin")).respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec())).mount(&server).await;
        let http = Http { client: reqwest::Client::new() };
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("file.part");
        let err =
            http.download(format!("{}/file.bin", server.uri()), dest.clone(), Some("00".repeat(32)), None).run(sipper::Sender::drain()).await.unwrap_err();
        assert!(matches!(err, CoreError::ChecksumMismatch { .. }));
        assert!(!dest.exists());
    }
}
