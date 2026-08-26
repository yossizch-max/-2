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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MatterProfile {
    pub matter_id: String, pub primary_event_date: Option<String>, pub primary_court_name: Option<String>,
    pub btl_claim_number: Option<String>, pub case_summary: Option<String>, pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MatterParty {
    pub id: String, pub matter_id: String, pub role: String, pub display_name: String,
    pub entity_kind: String, pub identifier: Option<String>, pub phone: Option<String>,
    pub email: Option<String>, pub address: Option<String>, pub notes: Option<String>,
    pub created_at: String, pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Workstream {
    pub id: String, pub matter_id: String, pub kind: String, pub status: String,
    pub notes: Option<String>, pub created_at: String, pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MatterRequirement {
    pub id: String, pub matter_id: String, pub requirement_key: String, pub status: String,
    pub relevance: String, pub priority: Option<String>,
    pub notes: Option<String>, pub created_at: String, pub updated_at: String,
}
