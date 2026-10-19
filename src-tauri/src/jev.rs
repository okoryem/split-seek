//! Jev client: asks Jev a yes/no question about one image and returns its 0–1 score.
//! See https://docs.typesafe.ai/images.md for the request shape.

use base64::Engine as _;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
const DEFAULT_MODEL: &str = "jev-latest";

pub struct Jev {
    http: reqwest::Client,
    api_key: String,
    model: String,
}

/// One answer from Jev.
#[derive(Debug, Clone)]
pub struct Score {
    /// 0 = clearly no, 1 = clearly yes.
    pub noul: f32,
    /// The exact model version that answered (e.g. "jev-1.14.0").
    pub model: String,
    pub input_tokens: u64,
}

#[derive(Debug)]
pub enum JevError {
    MissingKey,
    /// Worth retrying: rate limited (429) or a server error (5xx).
    Retryable(String),
    /// Not worth retrying: bad request, bad key, unreadable reply.
    Fatal(String),
}

impl std::fmt::Display for JevError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JevError::MissingKey => write!(f, "Jev API key missing — add it to .env"),
            JevError::Retryable(m) | JevError::Fatal(m) => write!(f, "{m}"),
        }
    }
}

impl Jev {
    /// Reads JEV_API_KEY and JEV_MODEL from the project's `.env` (or the environment).
    pub fn from_env() -> Result<Self, JevError> {
        let project_env = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.env");
        let _ = dotenvy::from_path(project_env).or_else(|_| dotenvy::dotenv().map(|_| ()));

        let api_key = std::env::var("JEV_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
            .ok_or(JevError::MissingKey)?;
        let model = std::env::var("JEV_MODEL")
            .ok()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| JevError::Fatal(e.to_string()))?;
        Ok(Self { http, api_key, model })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Asks: "Does this frame show: <phrase>?" for one JPEG.
    pub async fn score_jpeg(&self, jpeg: &[u8], phrase: &str) -> Result<Score, JevError> {
        let body = request_body(&self.model, jpeg, phrase);
        let resp = self
            .http
            .post(ENDPOINT)
            .bearer_auth(&self.api_key)
            .header("TypeSafe-Danger-Zone-Images", "true")
            .json(&body)
            .send()
            .await
            .map_err(|e| JevError::Retryable(format!("network error: {e}")))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if status.as_u16() == 429 || status.is_server_error() {
            return Err(JevError::Retryable(format!("{status}: {text}")));
        }
        if !status.is_success() {
            return Err(JevError::Fatal(format!("{status}: {text}")));
        }
        parse_score(&text)
    }

    /// Like `score_jpeg`, but waits and retries up to 3 times on rate limits and server errors.
    pub async fn score_jpeg_with_retry(&self, jpeg: &[u8], phrase: &str) -> Result<Score, JevError> {
        let mut wait = Duration::from_millis(500);
        for _ in 0..MAX_RETRIES {
            match self.score_jpeg(jpeg, phrase).await {
                Err(JevError::Retryable(_)) => {
                    tokio::time::sleep(wait).await;
                    wait *= 2;
                }
                other => return other,
            }
        }
        self.score_jpeg(jpeg, phrase).await
    }
}

const MAX_RETRIES: usize = 3;

fn request_body(model: &str, jpeg: &[u8], phrase: &str) -> Value {
    let data = base64::engine::general_purpose::STANDARD.encode(jpeg);
    json!({
        "model": model,
        "state": { "frame": "splitseek-frame" },
        "assets": {
            "splitseek-frame": { "type": "image", "format": "jpg", "data_base64": data }
        },
        "questions": {
            "match": {
                "type": "noul",
                "instructions": format!("Does the video frame `frame` show: {phrase}?"),
                "criteria": {
                    "true": "The frame clearly shows this",
                    "false": "The frame does not show this"
                }
            }
        }
    })
}

fn parse_score(text: &str) -> Result<Score, JevError> {
    let v: Value = serde_json::from_str(text)
        .map_err(|e| JevError::Fatal(format!("unreadable reply ({e}): {text}")))?;
    let noul = v["answers"]["match"]["noul"]
        .as_f64()
        .ok_or_else(|| JevError::Fatal(format!("no score in reply: {text}")))?;
    Ok(Score {
        noul: noul as f32,
        model: v["model"].as_str().unwrap_or("?").to_string(),
        input_tokens: v["usage"]["input_tokens"].as_u64().unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_score_from_a_reply() {
        let reply = r#"{"model":"jev-1.14.0","answers":{"match":{"type":"noul","noul":0.87}},"usage":{"input_tokens":412,"output_tokens":1}}"#;
        let s = parse_score(reply).unwrap();
        assert!((s.noul - 0.87).abs() < 1e-6);
        assert_eq!(s.model, "jev-1.14.0");
        assert_eq!(s.input_tokens, 412);
    }

    #[test]
    fn image_is_referenced_from_state() {
        let body = request_body("jev-latest", b"jpegbytes", "a dog");
        assert_eq!(body["state"]["frame"], "splitseek-frame");
        assert_eq!(body["assets"]["splitseek-frame"]["format"], "jpg");
        assert!(body["questions"]["match"]["instructions"].as_str().unwrap().contains("a dog"));
    }
}
