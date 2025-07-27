use thiserror::Error;
#[derive(Debug, Error)]
pub enum Error {
    #[error("Capturer Error: {0}")]
    CapturerError(String),
    #[error("OCR Error: {0}")]
    OcrError(String),
    #[error("Image Error: {0}")]
    ImageError(String),
}
