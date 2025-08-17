use crate::engine;
use bytes::Bytes;
use derive_more::From;
use image::math::Rect;
use reqwest::{Body, Client, multipart};
use serde::Deserialize;

pub struct OcrClient {
    client: Client,
    base_url: String,
}

impl OcrClient {
    pub fn new() -> Self {
        let client = Client::new();
        Self {
            client: client,
            base_url: "http://localhost:42069".to_string(),
        }
    }
}

pub struct OcrInput {
    // data of image in u16
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(From)]
pub struct OcrOutput {
    pub data: Vec<OcrOutputData>,
}
pub struct OcrOutputData {
    pub text: String,
    pub area: Rect,
}
impl From<OcrOutputData> for engine::AnalyzeCaptureAreaInput {
    fn from(from: OcrOutputData) -> Self {
        engine::AnalyzeCaptureAreaInput {
            text: from.text,
            area: from.area,
        }
    }
}
#[derive(Debug, Deserialize)]
struct OcrApiResult {
    result: Vec<OcrApiData>,
}
#[derive(Debug, Deserialize)]
struct OcrApiData {
    text: String,
    area: Area,
}

#[derive(Debug, Deserialize)]
struct Area {
    top: u32,
    left: u32,
    right: u32,
    bottom: u32,
}

impl OcrClient {
    pub async fn do_ocr(&self, input: OcrInput) -> Result<OcrOutput, OCRError> {
        // multipart::
        let data = input.data;
        let stream =
            futures_util::stream::once(
                async move { Ok::<Bytes, std::io::Error>(Bytes::from(data)) },
            );
        let part = multipart::Part::stream(Body::wrap_stream(stream))
            .file_name("file.png")
            .mime_str("image/png")
            .unwrap();
        let mupart = multipart::Form::new()
            .part("file", part)
            .text("width", input.width.to_string())
            .text("height", input.height.to_string());
        let result = self
            .client
            .post(self.base_url.clone() + "/ocr")
            .multipart(mupart)
            .send()
            .await?;
        // self.client.post("/ocr").
        let bytes = result.bytes().await?;
        // println! {"{}",status};
        // File
        // let mut file =
        //     File::create(format!("{}.json", chrono::Local::now().timestamp_millis())).unwrap();
        // _ = file.write_all(&bytes);

        let result: OcrApiResult = serde_json::from_slice(&bytes)?;
        let mut out = OcrOutput { data: Vec::new() };
        for (_, v) in result.result.iter().enumerate() {
            let clean: String = v
                .text
                .clone()
                .chars()
                .filter(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
                .collect();
            out.data.push(OcrOutputData {
                text: clean,
                area: Rect {
                    x: v.area.left,
                    y: v.area.top,
                    width: v.area.right - v.area.left,
                    height: v.area.bottom - v.area.top,
                },
            });
        }
        Ok(out)
    }
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum OCRError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("JSON parse failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Unknown error")]
    Unknown,
}

// impl std::fmt::Display for OCRError {
//     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//         match self {
//             OCRError::Json(e) => write!(f, "JSON error: {}", e),
//             OCRError::Io(e) => write!(f, "IO error: {}", e),
//             OCRError::RequestError(e) => write!(f, "Reqest error: {}", e),
//         }
//     }
// }
