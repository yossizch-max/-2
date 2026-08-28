use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("RECOVERY_REQUIRED")]
    RecoveryRequired,
    #[error("SOURCE_SHA_MISMATCH")]
    SourceShaMismatch,
    #[error("SOURCE_SNAPSHOT_CHANGED")]
    SourceSnapshotChanged,
    #[error("OCR_RUNTIME_MISSING")]
    OcrRuntimeMissing,
    #[error("UNSUPPORTED_FORMAT: {0}")]
    UnsupportedFormat(String),
    #[error("PDFTOTEXT_FAILED: {0}")]
    PdftotextFailed(String),
    #[error("RASTERIZATION_FAILED: {0}")]
    RasterizationFailed(String),
    #[error("OCR_FAILED: {0}")]
    OcrFailed(String),
    #[error("AI_CLIENT_EGRESS_NOT_APPROVED")]
    AiClientEgressNotApproved,
    #[error("AI_PROVIDER_REFUSAL")]
    AiProviderRefusal,
    #[error("INVALID_SOURCE_REFERENCE")]
    InvalidSourceReference,
    #[error("NO_APPROVED_RULE_FOR_CONTEXT")]
    NoApprovedRuleForContext,
    #[error("PDF_CONVERTER_UNAVAILABLE")]
    PdfConverterUnavailable,
    #[error("NOT_FOUND: {0}")]
    NotFound(String),
    #[error("VALIDATION: {0}")]
    Validation(String),
    #[error("DB: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("KEYRING: {0}")]
    Keyring(String),
    #[error("SERDE: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("HTTP: {0}")]
    Http(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
