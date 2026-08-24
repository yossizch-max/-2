use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Matter {
    pub id: String, pub title: String, pub internal_number: Option<String>,
    pub external_number: Option<String>, pub matter_type: String, pub status: String,
    pub workflow_stage: String, pub folder_path: Option<String>, pub document_count: i64,
    pub verified_fact_count: i64, pub pending_review_count: i64, pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewMatter {
    pub title: String, pub internal_number: Option<String>,
    pub matter_type: String, pub folder_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub kind: String, pub matter_id: Option<String>, pub id: String,
    pub title: String, pub subtitle: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DamageInput {
    pub key: String, pub cents: i64, pub source: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DamageResult {
    pub gross_cents: i64, pub deductions_cents: i64,
    pub net_cents: i64, pub integrity_sha256: String,
}
