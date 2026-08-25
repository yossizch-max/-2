//! Legal rules infrastructure (Phase A): a deterministic, constrained-DSL rule engine.
//! This module deliberately contains no Israeli substantive law - it only lets a
//! *governed, lawyer-approved* Ruleset be evaluated. See
//! `docs/RELEASE_GATES.md`/`TAHRIR_LEGAL_RULES_INFRASTRUCTURE_SPEC_20260825.md` for the
//! product rationale. This file holds two independent layers:
//!   - the DSL interpreter (`evaluate_conditions`/`execute_operations`, this section):
//!     pure functions, no DB, no network, no filesystem, no arbitrary code evaluation -
//!     just a fixed set of safe operators over a flat JSON register map.
//!   - the ruleset lifecycle and engine-run logic (further down): DB-backed, testable
//!     outside `tauri::State` via `&DbState`, same pattern as `authorities.rs`.
use crate::{db::DbState, error::{AppError, AppResult}};
use chrono::{Duration, NaiveDate, Utc};
use rusqlite::params;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// DSL: term resolution and typed accessors
// ---------------------------------------------------------------------------

/// A term is either a literal JSON value or `{"reg":"<name>"}`, a reference to a
/// named register. Anything else (including a JSON object without a "reg" key) is
/// treated as a literal - so a rule author can pass a literal object value if they
/// ever need to, at the cost of not being able to literally spell `{"reg": ...}`.
fn resolve<'a>(term: Option<&Value>, what: &str, registers: &'a Map<String, Value>) -> AppResult<std::borrow::Cow<'a, Value>> {
    let term = term.ok_or_else(|| AppError::Validation(format!("missing required field '{what}'")))?;
    if let Some(name) = term.as_object().and_then(|o| o.get("reg")).and_then(Value::as_str) {
        let value = registers.get(name)
            .ok_or_else(|| AppError::Validation(format!("unknown register '{name}' referenced by '{what}'")))?;
        return Ok(std::borrow::Cow::Borrowed(value));
    }
    Ok(std::borrow::Cow::Owned(term.clone()))
}

fn as_i64(v: &Value, what: &str) -> AppResult<i64> {
    v.as_i64().ok_or_else(|| AppError::Validation(format!("'{what}' must be an integer, got {v}")))
}

fn as_f64(v: &Value, what: &str) -> AppResult<f64> {
    v.as_f64().ok_or_else(|| AppError::Validation(format!("'{what}' must be a number, got {v}")))
}

fn as_str<'a>(v: &'a Value, what: &str) -> AppResult<&'a str> {
    v.as_str().ok_or_else(|| AppError::Validation(format!("'{what}' must be a string, got {v}")))
}

fn as_bool(v: &Value, what: &str) -> AppResult<bool> {
    v.as_bool().ok_or_else(|| AppError::Validation(format!("'{what}' must be a boolean, got {v}")))
}

fn as_date(v: &Value, what: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(as_str(v, what)?, "%Y-%m-%d")
        .map_err(|_| AppError::Validation(format!("'{what}' must be an ISO date (YYYY-MM-DD), got {v}")))
}

/// Fixed-point decimal multiply of integer cents by a decimal string like "0.75",
/// rounded half-up. Avoids floating point entirely so this is exactly reproducible.
fn multiply_cents_by_decimal(cents: i64, factor: &str, what: &str) -> AppResult<i64> {
    let (int_part, frac_part) = match factor.split_once('.') {
        Some((i, f)) => (i, f),
        None => (factor, ""),
    };
    if frac_part.len() > 9 || !int_part.bytes().all(|b| b.is_ascii_digit() || b == b'-')
        || !frac_part.bytes().all(|b| b.is_ascii_digit()) || int_part.is_empty() {
        return Err(AppError::Validation(format!("'{what}' must be a plain decimal string like \"0.75\", got \"{factor}\"")));
    }
    let scale: i128 = 10i128.pow(frac_part.len() as u32);
    let int_val: i128 = int_part.parse().map_err(|_| AppError::Validation(format!("'{what}' has an invalid integer part")))?;
    let frac_val: i128 = if frac_part.is_empty() { 0 } else {
        frac_part.parse().map_err(|_| AppError::Validation(format!("'{what}' has an invalid fractional part")))?
    };
    let sign = if int_val < 0 || factor.starts_with('-') { -1i128 } else { 1i128 };
    let scaled_factor = int_val.abs() * scale + frac_val;
    let scaled_factor = sign * scaled_factor;

    let product = cents as i128 * scaled_factor;
    // round-half-up on the scale we introduced, preserving sign
    let rounded = if product >= 0 {
        (product + scale / 2) / scale
    } else {
        -((-product + scale / 2) / scale)
    };
    i64::try_from(rounded).map_err(|_| AppError::Validation("multiply_decimal overflowed i64".into()))
}

fn compare_values(cmp: &str, left: &Value, right: &Value) -> AppResult<bool> {
    Ok(match cmp {
        "eq" => left == right,
        "neq" => left != right,
        "gt" | "gte" | "lt" | "lte" => {
            let ordering = if let (Some(a), Some(b)) = (left.as_f64(), right.as_f64()) {
                a.partial_cmp(&b)
            } else if let (Some(a), Some(b)) = (left.as_str(), right.as_str()) {
                Some(a.cmp(b))
            } else {
                return Err(AppError::Validation(format!(
                    "cannot compare {left} and {right}: both must be numbers, or both must be strings (ISO dates sort correctly as strings)"
                )));
            }.ok_or_else(|| AppError::Validation("values are not comparable".into()))?;
            match cmp {
                "gt" => ordering.is_gt(), "gte" => ordering.is_ge(),
                "lt" => ordering.is_lt(), "lte" => ordering.is_le(),
                _ => unreachable!(),
            }
        }
        other => return Err(AppError::Validation(format!("unknown comparison op '{other}'"))),
    })
}

// ---------------------------------------------------------------------------
// DSL: conditions
// ---------------------------------------------------------------------------

/// Every condition must match (implicit AND) against `context` for the rule to apply.
/// A condition referencing a field absent from `context` simply does not match (the
/// rule doesn't apply to this input shape) - it is not an error. Use the
/// `require_input` operation if a rule's *operations* actually depend on a field
/// being present once the rule has already matched.
pub fn evaluate_conditions(conditions_json: &str, context: &Map<String, Value>) -> AppResult<bool> {
    let conditions: Vec<Value> = serde_json::from_str(conditions_json)
        .map_err(|e| AppError::Validation(format!("malformed conditions_json: {e}")))?;
    for cond in &conditions {
        let obj = cond.as_object().ok_or_else(|| AppError::Validation("each condition must be a JSON object".into()))?;
        let field = obj.get("field").and_then(Value::as_str)
            .ok_or_else(|| AppError::Validation("condition missing 'field'".into()))?;
        let op = obj.get("op").and_then(Value::as_str)
            .ok_or_else(|| AppError::Validation("condition missing 'op'".into()))?;
        let expected = obj.get("value")
            .ok_or_else(|| AppError::Validation("condition missing 'value'".into()))?;
        let actual = context.get(field);

        let matched = if op == "in" {
            let arr = expected.as_array()
                .ok_or_else(|| AppError::Validation("'in' condition's value must be an array".into()))?;
            matches!(actual, Some(a) if arr.contains(a))
        } else {
            match actual {
                Some(a) => compare_values(op, a, expected)?,
                None => false,
            }
        };
        if !matched { return Ok(false); }
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// DSL: operations
// ---------------------------------------------------------------------------

/// Runs `operation_json`'s steps in order against a working register map seeded from
/// `context`, returning the final registers plus a step-by-step trace (what each step
/// read and wrote) so a committed engine run can always explain itself.
pub fn execute_operations(operation_json: &str, context: &Map<String, Value>) -> AppResult<(Map<String, Value>, Vec<Value>)> {
    let steps: Vec<Value> = serde_json::from_str(operation_json)
        .map_err(|e| AppError::Validation(format!("malformed operation_json: {e}")))?;
    let mut registers = context.clone();
    let mut trace = Vec::with_capacity(steps.len());

    for (index, step) in steps.iter().enumerate() {
        let obj = step.as_object().ok_or_else(|| AppError::Validation(format!("operation step {index} must be a JSON object")))?;
        let op = obj.get("op").and_then(Value::as_str)
            .ok_or_else(|| AppError::Validation(format!("operation step {index} missing 'op'")))?;

        if op == "require_input" {
            let field = obj.get("field").and_then(Value::as_str)
                .ok_or_else(|| AppError::Validation(format!("step {index} 'require_input' missing 'field'")))?;
            match registers.get(field) {
                Some(v) if !v.is_null() => {}
                _ => return Err(AppError::Validation(format!("required input '{field}' is missing"))),
            }
            trace.push(serde_json::json!({"step":index,"op":"require_input","field":field}));
            continue;
        }

        let into = obj.get("into").and_then(Value::as_str)
            .ok_or_else(|| AppError::Validation(format!("step {index} ('{op}') missing 'into'")))?;

        let output: Value = match op {
            "compare" => {
                let left = resolve(obj.get("left"), "left", &registers)?;
                let right = resolve(obj.get("right"), "right", &registers)?;
                let cmp = as_str(obj.get("cmp").ok_or_else(|| AppError::Validation(format!("step {index} 'compare' missing 'cmp'")))?, "cmp")?;
                Value::Bool(compare_values(cmp, &left, &right)?)
            }
            "add_days" | "subtract_days" => {
                let from = resolve(obj.get("from"), "from", &registers)?;
                let days = resolve(obj.get("days"), "days", &registers)?;
                let date = as_date(&from, "from")?;
                let n = as_i64(&days, "days")?;
                let shifted = if op == "add_days" { date + Duration::days(n) } else { date - Duration::days(n) };
                Value::String(shifted.format("%Y-%m-%d").to_string())
            }
            "add_amount" | "subtract_amount" => {
                let from = resolve(obj.get("from"), "from", &registers)?;
                let amount = resolve(obj.get("amount"), "amount", &registers)?;
                let a = as_i64(&from, "from")?;
                let b = as_i64(&amount, "amount")?;
                let result = if op == "add_amount" { a.checked_add(b) } else { a.checked_sub(b) }
                    .ok_or_else(|| AppError::Validation(format!("step {index} ('{op}') overflowed i64")))?;
                Value::Number(result.into())
            }
            "multiply_decimal" => {
                let from = resolve(obj.get("from"), "from", &registers)?;
                let factor = resolve(obj.get("factor"), "factor", &registers)?;
                let cents = as_i64(&from, "from")?;
                let factor_str = as_str(&factor, "factor")?;
                Value::Number(multiply_cents_by_decimal(cents, factor_str, "factor")?.into())
            }
            "cap" | "floor" => {
                let value = resolve(obj.get("value"), "value", &registers)?;
                let bound_key = if op == "cap" { "max" } else { "min" };
                let bound = resolve(obj.get(bound_key), bound_key, &registers)?;
                let v = as_f64(&value, "value")?;
                let b = as_f64(&bound, bound_key)?;
                let result = if op == "cap" { v.min(b) } else { v.max(b) };
                if value.is_i64() && bound.is_i64() {
                    Value::Number((result as i64).into())
                } else {
                    serde_json::Number::from_f64(result).map(Value::Number)
                        .ok_or_else(|| AppError::Validation(format!("step {index} ('{op}') produced a non-finite number")))?
                }
            }
            "choose" => {
                let when = resolve(obj.get("when"), "when", &registers)?;
                let branch_key = if as_bool(&when, "when")? { "then" } else { "else" };
                resolve(obj.get(branch_key), branch_key, &registers)?.into_owned()
            }
            other => return Err(AppError::Validation(format!("unknown operation '{other}' at step {index}"))),
        };

        trace.push(serde_json::json!({"step":index,"op":op,"into":into,"output":output}));
        registers.insert(into.to_string(), output);
    }

    Ok((registers, trace))
}

// ---------------------------------------------------------------------------
// Ruleset lifecycle (DB-backed; testable outside tauri::State, same pattern as
// authorities.rs). Nothing here decides what a rule *means* - only whether a
// Ruleset is properly sourced, tested and governed before it may be used.
// ---------------------------------------------------------------------------

pub fn create_ruleset(
    db: &DbState, engine_kind: &str, jurisdiction: &str, title: &str, version: &str,
    effective_from: Option<&str>, effective_to: Option<&str>, description: Option<&str>, created_by: Option<&str>,
) -> AppResult<String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO legal_rulesets(
                id,engine_kind,jurisdiction,title,version,status,effective_from,effective_to,
                description,created_at,created_by
             ) VALUES(?1,?2,?3,?4,?5,'draft',?6,?7,?8,?9,?10)",
            params![id, engine_kind, jurisdiction, title, version, effective_from, effective_to, description, now, created_by],
        )?;
        Ok(())
    })?;
    Ok(id)
}

fn require_draft(conn: &rusqlite::Connection, ruleset_id: &str) -> AppResult<()> {
    let status: String = conn.query_row(
        "SELECT status FROM legal_rulesets WHERE id=?1", [ruleset_id], |r| r.get(0),
    ).map_err(|_| AppError::Validation("ruleset not found".into()))?;
    if status != "draft" {
        return Err(AppError::Validation(format!("ruleset is '{status}', not 'draft' - it cannot be edited")));
    }
    Ok(())
}

pub fn update_draft_ruleset(
    db: &DbState, ruleset_id: &str, title: Option<&str>, effective_from: Option<&str>,
    effective_to: Option<&str>, description: Option<&str>,
) -> AppResult<()> {
    db.write(|conn| {
        require_draft(conn, ruleset_id)?;
        conn.execute(
            "UPDATE legal_rulesets SET
                title=coalesce(?2,title), effective_from=?3, effective_to=?4, description=?5
             WHERE id=?1",
            params![ruleset_id, title, effective_from, effective_to, description],
        )?;
        Ok(())
    })
}

/// A source bound to a real in-app document is verified immediately (its SHA256 is
/// read straight from `document_versions`, not trusted from the caller) - it can't
/// lie about what the document says because we hash the document ourselves. A
/// citation-only source (no `document_version_id`) is only marked verified if the
/// caller explicitly names a `verified_by` - i.e. a lawyer's deliberate act, not a
/// default. Either way `source_sha256` is always computed here, never accepted as
/// input, so it can't be forged.
pub fn add_source(
    db: &DbState, ruleset_id: &str, source_kind: &str, citation: &str, pinpoint: Option<&str>,
    document_version_id: Option<&str>, document_page_id: Option<&str>, verified_by: Option<&str>,
) -> AppResult<String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        require_draft(conn, ruleset_id)?;

        let (source_sha256, verified_at) = if let Some(version_id) = document_version_id {
            let content_sha256: String = conn.query_row(
                "SELECT content_sha256 FROM document_versions WHERE id=?1", [version_id], |r| r.get(0),
            ).map_err(|_| AppError::InvalidSourceReference)?;
            if let Some(page_id) = document_page_id {
                let belongs: i64 = conn.query_row(
                    "SELECT count(*) FROM document_pages WHERE id=?1 AND document_version_id=?2",
                    params![page_id, version_id], |r| r.get(0),
                )?;
                if belongs == 0 { return Err(AppError::InvalidSourceReference); }
            }
            (content_sha256, Some(now.clone()))
        } else {
            let hash = hex::encode(Sha256::digest(format!("{citation}:{}", pinpoint.unwrap_or("")).as_bytes()));
            (hash, verified_by.map(|_| now.clone()))
        };

        conn.execute(
            "INSERT INTO legal_ruleset_sources(
                id,ruleset_id,source_kind,citation,pinpoint,document_version_id,document_page_id,
                source_sha256,verified_at,verified_by,created_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![id, ruleset_id, source_kind, citation, pinpoint, document_version_id, document_page_id,
                source_sha256, verified_at, verified_by, now],
        )?;
        Ok(())
    })?;
    Ok(id)
}

/// Structural validation only (valid JSON, right shape) - not full semantic execution,
/// since a rule's operations may legitimately reference input fields that don't exist
/// in any particular context yet. Real correctness is proven by `run_tests`, which is
/// mandatory before approval - not by a static check here.
fn validate_rule_shape(conditions_json: &str, operation_json: &str) -> AppResult<()> {
    let conditions: Vec<Value> = serde_json::from_str(conditions_json)
        .map_err(|e| AppError::Validation(format!("malformed conditions_json: {e}")))?;
    for c in &conditions {
        let obj = c.as_object().ok_or_else(|| AppError::Validation("each condition must be a JSON object".into()))?;
        for key in ["field", "op", "value"] {
            if !obj.contains_key(key) { return Err(AppError::Validation(format!("condition missing '{key}'"))); }
        }
    }
    let steps: Vec<Value> = serde_json::from_str(operation_json)
        .map_err(|e| AppError::Validation(format!("malformed operation_json: {e}")))?;
    if steps.is_empty() {
        return Err(AppError::Validation("a rule must have at least one operation step".into()));
    }
    for (index, s) in steps.iter().enumerate() {
        let obj = s.as_object().ok_or_else(|| AppError::Validation(format!("operation step {index} must be a JSON object")))?;
        if !obj.get("op").is_some_and(Value::is_string) {
            return Err(AppError::Validation(format!("operation step {index} missing 'op'")));
        }
    }
    Ok(())
}

pub fn add_rule(
    db: &DbState, ruleset_id: &str, rule_key: &str, rule_type: &str, priority: i64,
    conditions_json: &str, operation_json: &str, explanation_template: Option<&str>, source_id: Option<&str>,
) -> AppResult<String> {
    validate_rule_shape(conditions_json, operation_json)?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        require_draft(conn, ruleset_id)?;
        if let Some(sid) = source_id {
            let belongs: i64 = conn.query_row(
                "SELECT count(*) FROM legal_ruleset_sources WHERE id=?1 AND ruleset_id=?2",
                params![sid, ruleset_id], |r| r.get(0),
            )?;
            if belongs == 0 { return Err(AppError::InvalidSourceReference); }
        }
        conn.execute(
            "INSERT INTO legal_rules(
                id,ruleset_id,rule_key,rule_type,priority,conditions_json,operation_json,
                explanation_template,source_id,created_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![id, ruleset_id, rule_key, rule_type, priority, conditions_json, operation_json,
                explanation_template, source_id, now],
        )?;
        Ok(())
    })?;
    Ok(id)
}

pub fn add_test_case(
    db: &DbState, ruleset_id: &str, name: &str, input_json: &str, expected_output_json: &str,
) -> AppResult<String> {
    let _: Value = serde_json::from_str(input_json).map_err(|e| AppError::Validation(format!("malformed input_json: {e}")))?;
    let _: Value = serde_json::from_str(expected_output_json).map_err(|e| AppError::Validation(format!("malformed expected_output_json: {e}")))?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        require_draft(conn, ruleset_id)?;
        conn.execute(
            "INSERT INTO legal_rule_test_cases(id,ruleset_id,name,input_json,expected_output_json,review_status,created_at)
             VALUES(?1,?2,?3,?4,?5,'draft',?6)",
            params![id, ruleset_id, name, input_json, expected_output_json, now],
        )?;
        Ok(())
    })?;
    Ok(id)
}

pub fn review_test_case(db: &DbState, ruleset_id: &str, test_case_id: &str, approved: bool, reviewed_by: Option<&str>) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        require_draft(conn, ruleset_id)?;
        let changed = conn.execute(
            "UPDATE legal_rule_test_cases SET review_status=?3,reviewed_by=?4,reviewed_at=?5
             WHERE id=?1 AND ruleset_id=?2",
            params![test_case_id, ruleset_id, if approved { "approved" } else { "rejected" }, reviewed_by, now],
        )?;
        if changed != 1 { return Err(AppError::Validation("test case not found".into())); }
        Ok(())
    })
}

/// Runs every rule in the ruleset (priority ascending, first match wins) against every
/// test case's `input_json`, and checks the resulting registers contain (at least)
/// every key/value in `expected_output_json` - plus, if present, a special
/// `"matchedRuleKey"` key checked against which rule actually fired. A test case whose
/// rule finding or DSL execution errors is reported as a failure with that error as the
/// reason, never silently skipped. Returns one result per test case, `(name, passed, detail)`.
pub fn run_tests(db: &DbState, ruleset_id: &str) -> AppResult<Vec<(String, bool, String)>> {
    db.read(|conn| {
        let mut rule_stmt = conn.prepare(
            "SELECT rule_key,conditions_json,operation_json FROM legal_rules WHERE ruleset_id=?1 ORDER BY priority ASC"
        )?;
        let rules: Vec<(String, String, String)> = rule_stmt.query_map([ruleset_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?.collect::<Result<Vec<_>, _>>()?;

        let mut tc_stmt = conn.prepare(
            "SELECT name,input_json,expected_output_json FROM legal_rule_test_cases WHERE ruleset_id=?1"
        )?;
        let test_cases: Vec<(String, String, String)> = tc_stmt.query_map([ruleset_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?.collect::<Result<Vec<_>, _>>()?;

        let mut results = Vec::with_capacity(test_cases.len());
        for (name, input_json, expected_json) in test_cases {
            results.push(run_single_test(&rules, &name, &input_json, &expected_json));
        }
        Ok(results)
    })
}

fn run_single_test(rules: &[(String, String, String)], name: &str, input_json: &str, expected_json: &str) -> (String, bool, String) {
    let outcome: AppResult<(bool, String)> = (|| {
        let input: Value = serde_json::from_str(input_json).map_err(|e| AppError::Validation(format!("malformed test input_json: {e}")))?;
        let context = input.as_object().cloned().ok_or_else(|| AppError::Validation("test input_json must be a JSON object".into()))?;
        let expected: Value = serde_json::from_str(expected_json).map_err(|e| AppError::Validation(format!("malformed expected_output_json: {e}")))?;
        let expected_obj = expected.as_object().ok_or_else(|| AppError::Validation("expected_output_json must be a JSON object".into()))?;

        let mut matched: Option<(&str, &Map<String, Value>)> = None;
        for (rule_key, conditions_json, _) in rules {
            if evaluate_conditions(conditions_json, &context)? {
                matched = Some((rule_key, &context));
                break;
            }
        }
        let Some((matched_key, _)) = matched else {
            // a test case asserting "no rule should match" uses an empty expected_output_json
            return Ok((expected_obj.is_empty(), "no rule matched this test's input".to_string()));
        };
        let operation_json = &rules.iter().find(|(k, _, _)| k == matched_key).unwrap().2;
        let (registers, _trace) = execute_operations(operation_json, &context)?;

        for (key, expected_value) in expected_obj {
            if key == "matchedRuleKey" {
                if expected_value.as_str() != Some(matched_key) {
                    return Ok((false, format!("expected rule '{}' to match, but '{matched_key}' matched", expected_value.as_str().unwrap_or("?"))));
                }
                continue;
            }
            match registers.get(key) {
                Some(actual) if actual == expected_value => {}
                Some(actual) => return Ok((false, format!("register '{key}': expected {expected_value}, got {actual}"))),
                None => return Ok((false, format!("register '{key}' was never set"))),
            }
        }
        Ok((true, format!("matched rule '{matched_key}'")))
    })();

    match outcome {
        Ok((passed, detail)) => (name.to_string(), passed, detail),
        Err(e) => (name.to_string(), false, e.to_string()),
    }
}

pub fn submit_for_review(db: &DbState, ruleset_id: &str) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        require_draft(conn, ruleset_id)?;
        let (rule_count, source_count, test_count): (i64, i64, i64) = conn.query_row(
            "SELECT
                (SELECT count(*) FROM legal_rules WHERE ruleset_id=?1),
                (SELECT count(*) FROM legal_ruleset_sources WHERE ruleset_id=?1),
                (SELECT count(*) FROM legal_rule_test_cases WHERE ruleset_id=?1)",
            [ruleset_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        if rule_count == 0 || source_count == 0 || test_count == 0 {
            return Err(AppError::Validation(
                "a ruleset needs at least one rule, one source and one test case before it can be submitted for review".into()
            ));
        }
        let changed = conn.execute(
            "UPDATE legal_rulesets SET status='under_review',submitted_for_review_at=?2 WHERE id=?1 AND status='draft'",
            params![ruleset_id, now],
        )?;
        if changed != 1 { return Err(AppError::Validation("ruleset not submittable".into())); }
        Ok(())
    })
}

/// Builds a deterministic structural fingerprint of everything that makes up a
/// ruleset's meaning: every rule (ordered by rule_key), every source, every test case.
/// Two rulesets with identical content hash identically; changing anything - a
/// condition, a source, an approved test case - changes the hash.
fn canonical_content(conn: &rusqlite::Connection, ruleset_id: &str) -> AppResult<String> {
    let (engine_kind, jurisdiction, title, version): (String, String, String, String) = conn.query_row(
        "SELECT engine_kind,jurisdiction,title,version FROM legal_rulesets WHERE id=?1", [ruleset_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;

    let mut rule_stmt = conn.prepare(
        "SELECT rule_key,rule_type,priority,conditions_json,operation_json,source_id
         FROM legal_rules WHERE ruleset_id=?1 ORDER BY rule_key"
    )?;
    let rules: Vec<Value> = rule_stmt.query_map([ruleset_id], |r| Ok(json!({
        "ruleKey": r.get::<_,String>(0)?, "ruleType": r.get::<_,String>(1)?, "priority": r.get::<_,i64>(2)?,
        "conditions": r.get::<_,String>(3)?, "operations": r.get::<_,String>(4)?, "sourceId": r.get::<_,Option<String>>(5)?,
    })))?.collect::<Result<Vec<_>, _>>()?;

    let mut source_stmt = conn.prepare(
        "SELECT id,citation,pinpoint,source_sha256 FROM legal_ruleset_sources WHERE ruleset_id=?1 ORDER BY id"
    )?;
    let sources: Vec<Value> = source_stmt.query_map([ruleset_id], |r| Ok(json!({
        "id": r.get::<_,String>(0)?, "citation": r.get::<_,String>(1)?,
        "pinpoint": r.get::<_,Option<String>>(2)?, "sha256": r.get::<_,String>(3)?,
    })))?.collect::<Result<Vec<_>, _>>()?;

    let mut tc_stmt = conn.prepare(
        "SELECT id,name,input_json,expected_output_json,review_status FROM legal_rule_test_cases WHERE ruleset_id=?1 ORDER BY id"
    )?;
    let test_cases: Vec<Value> = tc_stmt.query_map([ruleset_id], |r| Ok(json!({
        "id": r.get::<_,String>(0)?, "name": r.get::<_,String>(1)?,
        "input": r.get::<_,String>(2)?, "expected": r.get::<_,String>(3)?, "status": r.get::<_,String>(4)?,
    })))?.collect::<Result<Vec<_>, _>>()?;

    Ok(json!({
        "engineKind": engine_kind, "jurisdiction": jurisdiction, "title": title, "version": version,
        "rules": rules, "sources": sources, "testCases": test_cases,
    }).to_string())
}

/// Approving a ruleset is the one moment its whole content is locked forever (short of
/// supersession). Every invariant is re-checked here, not trusted from earlier steps:
/// at least one verified, non-stale source; every rule cites a verified source; at
/// least one test case exists and ALL of them are review_status='approved' AND
/// currently pass a fresh deterministic run (not a cached prior result).
pub fn approve_ruleset(db: &DbState, ruleset_id: &str, approved_by: Option<&str>) -> AppResult<String> {
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        require_draft_or_under_review(conn, ruleset_id)?;

        let verified_source_count: i64 = conn.query_row(
            "SELECT count(*) FROM legal_ruleset_sources s WHERE s.ruleset_id=?1 AND s.verified_at IS NOT NULL
             AND (s.document_version_id IS NULL OR EXISTS(
                SELECT 1 FROM document_versions v WHERE v.id=s.document_version_id AND v.stale=0
             ))",
            [ruleset_id], |r| r.get(0),
        )?;
        if verified_source_count == 0 {
            return Err(AppError::Validation("a ruleset needs at least one currently verified, non-stale source before it can be approved".into()));
        }

        let unsourced_rules: i64 = conn.query_row(
            "SELECT count(*) FROM legal_rules r WHERE r.ruleset_id=?1 AND (
                r.source_id IS NULL OR NOT EXISTS(
                    SELECT 1 FROM legal_ruleset_sources s WHERE s.id=r.source_id AND s.verified_at IS NOT NULL
                    AND (s.document_version_id IS NULL OR EXISTS(
                        SELECT 1 FROM document_versions v WHERE v.id=s.document_version_id AND v.stale=0
                    ))
                )
             )",
            [ruleset_id], |r| r.get(0),
        )?;
        if unsourced_rules > 0 {
            return Err(AppError::Validation("every rule must cite a currently verified, non-stale source before the ruleset can be approved".into()));
        }

        let (test_count, unapproved_test_count): (i64, i64) = conn.query_row(
            "SELECT count(*), sum(CASE WHEN review_status<>'approved' THEN 1 ELSE 0 END)
             FROM legal_rule_test_cases WHERE ruleset_id=?1",
            [ruleset_id], |r| Ok((r.get(0)?, r.get::<_,Option<i64>>(1)?.unwrap_or(0))),
        )?;
        if test_count == 0 {
            return Err(AppError::Validation("a ruleset needs at least one test case before it can be approved".into()));
        }
        if unapproved_test_count > 0 {
            return Err(AppError::Validation(format!("{unapproved_test_count} test case(s) are not yet reviewed/approved")));
        }

        Ok(())
    })?;

    // Fresh, non-cached test execution - run_tests opens its own read connection,
    // executed after the write-guarded structural checks above so we know there's at
    // least one approved test case to run.
    let results = run_tests(db, ruleset_id)?;
    if results.is_empty() || results.iter().any(|(_, passed, _)| !passed) {
        let failing: Vec<String> = results.iter().filter(|(_, p, _)| !p)
            .map(|(name, _, detail)| format!("{name}: {detail}")).collect();
        return Err(AppError::Validation(format!(
            "the ruleset's test suite does not pass right now, refusing to approve: {}",
            failing.join("; ")
        )));
    }

    db.write(|conn| {
        require_draft_or_under_review(conn, ruleset_id)?;
        let canonical = canonical_content(conn, ruleset_id)?;
        let integrity_sha256 = hex::encode(Sha256::digest(canonical.as_bytes()));
        let changed = conn.execute(
            "UPDATE legal_rulesets SET status='approved',approved_at=?2,approved_by=?3,integrity_sha256=?4
             WHERE id=?1 AND status IN ('draft','under_review')",
            params![ruleset_id, now, approved_by, integrity_sha256],
        )?;
        if changed != 1 { return Err(AppError::Validation("ruleset not approvable".into())); }
        Ok(integrity_sha256)
    })
}

fn require_draft_or_under_review(conn: &rusqlite::Connection, ruleset_id: &str) -> AppResult<()> {
    let status: String = conn.query_row(
        "SELECT status FROM legal_rulesets WHERE id=?1", [ruleset_id], |r| r.get(0),
    ).map_err(|_| AppError::Validation("ruleset not found".into()))?;
    if status != "draft" && status != "under_review" {
        return Err(AppError::Validation(format!("ruleset is '{status}' - it cannot be approved from this state")));
    }
    Ok(())
}

/// Supersession never deletes: the old ruleset's row (and everything approved with it)
/// is retained forever, only its `status`/`superseded_by` change - the one mutation the
/// approved-ruleset trigger explicitly permits. `new_ruleset_id` must itself already be
/// an approved ruleset for the same engine/jurisdiction, so supersession always points
/// at another governed asset, never a draft.
pub fn supersede_ruleset(db: &DbState, old_ruleset_id: &str, new_ruleset_id: &str) -> AppResult<()> {
    db.write(|conn| {
        let (old_status, engine_kind, jurisdiction): (String, String, String) = conn.query_row(
            "SELECT status,engine_kind,jurisdiction FROM legal_rulesets WHERE id=?1", [old_ruleset_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).map_err(|_| AppError::Validation("ruleset not found".into()))?;
        if old_status != "approved" {
            return Err(AppError::Validation("only an approved ruleset can be superseded".into()));
        }
        let (new_status, new_engine_kind, new_jurisdiction): (String, String, String) = conn.query_row(
            "SELECT status,engine_kind,jurisdiction FROM legal_rulesets WHERE id=?1", [new_ruleset_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).map_err(|_| AppError::Validation("replacement ruleset not found".into()))?;
        if new_status != "approved" || new_engine_kind != engine_kind || new_jurisdiction != jurisdiction {
            return Err(AppError::Validation(
                "the replacement ruleset must itself be approved, for the same engine and jurisdiction".into()
            ));
        }
        let changed = conn.execute(
            "UPDATE legal_rulesets SET status='superseded',superseded_by=?2 WHERE id=?1 AND status='approved'",
            params![old_ruleset_id, new_ruleset_id],
        )?;
        if changed != 1 { return Err(AppError::Validation("ruleset not supersedable".into())); }
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Engine runs: preview (no persistence) and commit (immutable, persisted trace)
// ---------------------------------------------------------------------------

pub struct EngineRunOutcome {
    pub matched_rule_key: String,
    pub explanation: String,
    pub registers: Map<String, Value>,
    pub trace: Vec<Value>,
    pub ruleset_version: String,
    pub ruleset_integrity_sha256: String,
}

/// `{name}` placeholders in `template` are substituted from `registers`/`context`
/// (registers take precedence, since they include the context plus everything
/// computed). A placeholder with no matching value is left as-is rather than causing
/// an error - an explanation template is prose, not something that should fail closed.
fn render_explanation(template: Option<&str>, registers: &Map<String, Value>) -> String {
    let Some(template) = template else { return String::new(); };
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if let Some(end) = template[i..].find('}') {
                let key = &template[i + 1..i + end];
                match registers.get(key) {
                    Some(Value::String(s)) => out.push_str(s),
                    Some(other) => out.push_str(&other.to_string()),
                    None => out.push_str(&template[i..=i + end]),
                }
                i += end + 1;
                continue;
            }
        }
        let ch = template[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn run_ruleset_against_context(conn: &rusqlite::Connection, ruleset_id: &str, context: &Map<String, Value>) -> AppResult<EngineRunOutcome> {
    let (status, version, integrity_sha256): (String, String, Option<String>) = conn.query_row(
        "SELECT status,version,integrity_sha256 FROM legal_rulesets WHERE id=?1", [ruleset_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).map_err(|_| AppError::Validation("ruleset not found".into()))?;
    if status != "approved" {
        return Err(AppError::Validation(format!("ruleset is '{status}', not 'approved' - it cannot be used for a legal engine run")));
    }
    let integrity_sha256 = integrity_sha256.ok_or_else(|| AppError::Validation("approved ruleset is missing its integrity hash".into()))?;

    let mut stmt = conn.prepare(
        "SELECT rule_key,conditions_json,operation_json,explanation_template,source_id
         FROM legal_rules WHERE ruleset_id=?1 ORDER BY priority ASC"
    )?;
    let rules: Vec<(String, String, String, Option<String>, Option<String>)> = stmt.query_map([ruleset_id], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    })?.collect::<Result<Vec<_>, _>>()?;

    for (rule_key, conditions_json, operation_json, explanation_template, source_id) in &rules {
        if !evaluate_conditions(conditions_json, context)? { continue; }

        if let Some(source_id) = source_id {
            let stale: i64 = conn.query_row(
                "SELECT count(*) FROM legal_ruleset_sources s
                 WHERE s.id=?1 AND s.document_version_id IS NOT NULL AND EXISTS(
                    SELECT 1 FROM document_versions v WHERE v.id=s.document_version_id AND v.stale=1
                 )",
                [source_id], |r| r.get(0),
            )?;
            if stale > 0 {
                return Err(AppError::Validation(format!(
                    "rule '{rule_key}' matched, but its source has gone stale since the ruleset was approved - it cannot be used until the ruleset is re-approved or superseded"
                )));
            }
        }

        let (registers, trace) = execute_operations(operation_json, context)?;
        let explanation = render_explanation(explanation_template.as_deref(), &registers);
        return Ok(EngineRunOutcome {
            matched_rule_key: rule_key.clone(), explanation, registers, trace,
            ruleset_version: version, ruleset_integrity_sha256: integrity_sha256,
        });
    }
    Err(AppError::NoApprovedRuleForContext)
}

/// Computes a result without persisting anything - lets the UI show "here's what the
/// approved ruleset would produce" before a lawyer commits it.
pub fn preview_engine_run(db: &DbState, ruleset_id: &str, context_json: &str) -> AppResult<EngineRunOutcome> {
    let context: Value = serde_json::from_str(context_json).map_err(|e| AppError::Validation(format!("malformed input context: {e}")))?;
    let context = context.as_object().cloned().ok_or_else(|| AppError::Validation("input context must be a JSON object".into()))?;
    db.read(|conn| run_ruleset_against_context(conn, ruleset_id, &context))
}

/// Never trusts a previewed result: recomputes from scratch server-side against the
/// ruleset as it exists right now, then persists an immutable trace row. matter_id is
/// bound directly (not inferred), so a run always belongs to a specific matter.
pub fn commit_engine_run(db: &DbState, matter_id: &str, engine_kind: &str, ruleset_id: &str, context_json: &str) -> AppResult<String> {
    let context: Value = serde_json::from_str(context_json).map_err(|e| AppError::Validation(format!("malformed input context: {e}")))?;
    let context_obj = context.as_object().cloned().ok_or_else(|| AppError::Validation("input context must be a JSON object".into()))?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        let outcome = run_ruleset_against_context(conn, ruleset_id, &context_obj)?;
        let result_json = json!({"matchedRuleKey": outcome.matched_rule_key, "explanation": outcome.explanation, "registers": outcome.registers}).to_string();
        let trace_json = Value::Array(outcome.trace).to_string();
        conn.execute(
            "INSERT INTO legal_engine_runs(
                id,matter_id,engine_kind,ruleset_id,ruleset_version,input_snapshot_json,result_json,
                trace_json,ruleset_integrity_sha256,status,created_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'committed',?10)",
            params![id, matter_id, engine_kind, ruleset_id, outcome.ruleset_version, context_json, result_json,
                trace_json, outcome.ruleset_integrity_sha256, now],
        )?;
        Ok(())
    })?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx(pairs: &[(&str, Value)]) -> Map<String, Value> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn conditions_all_must_match_and_missing_field_is_a_non_match_not_an_error() {
        let context = ctx(&[("procedure_type", json!("tort_claim")), ("amount", json!(500))]);
        let conditions = json!([
            {"field":"procedure_type","op":"eq","value":"tort_claim"},
            {"field":"amount","op":"gte","value":100}
        ]).to_string();
        assert!(evaluate_conditions(&conditions, &context).unwrap());

        let mismatched = json!([{"field":"procedure_type","op":"eq","value":"other"}]).to_string();
        assert!(!evaluate_conditions(&mismatched, &context).unwrap());

        let missing_field = json!([{"field":"nonexistent","op":"eq","value":"x"}]).to_string();
        assert!(!evaluate_conditions(&missing_field, &context).unwrap(), "a missing field is a non-match, not an error");
    }

    #[test]
    fn conditions_in_operator_checks_membership() {
        let context = ctx(&[("court_or_forum", json!("magistrates"))]);
        let conditions = json!([{"field":"court_or_forum","op":"in","value":["magistrates","district"]}]).to_string();
        assert!(evaluate_conditions(&conditions, &context).unwrap());
    }

    #[test]
    fn malformed_conditions_json_fails_closed() {
        let context = ctx(&[]);
        assert!(evaluate_conditions("not json", &context).is_err());
        assert!(evaluate_conditions(r#"[{"field":"x"}]"#, &context).is_err(), "a condition missing 'op'/'value' must error, not be silently skipped");
    }

    #[test]
    fn add_days_and_subtract_days_shift_iso_dates() {
        let context = ctx(&[("trigger_date", json!("2026-01-10"))]);
        let ops = json!([
            {"op":"add_days","from":{"reg":"trigger_date"},"days":30,"into":"result"}
        ]).to_string();
        let (registers, trace) = execute_operations(&ops, &context).unwrap();
        assert_eq!(registers["result"], json!("2026-02-09"));
        assert_eq!(trace.len(), 1);

        let ops = json!([
            {"op":"subtract_days","from":{"reg":"trigger_date"},"days":5,"into":"result"}
        ]).to_string();
        let (registers, _) = execute_operations(&ops, &context).unwrap();
        assert_eq!(registers["result"], json!("2026-01-05"));
    }

    #[test]
    fn add_amount_subtract_amount_and_chained_registers() {
        let context = ctx(&[("base_cents", json!(10_000))]);
        let ops = json!([
            {"op":"add_amount","from":{"reg":"base_cents"},"amount":500,"into":"step1"},
            {"op":"subtract_amount","from":{"reg":"step1"},"amount":200,"into":"result"}
        ]).to_string();
        let (registers, trace) = execute_operations(&ops, &context).unwrap();
        assert_eq!(registers["result"], json!(10_300));
        assert_eq!(trace.len(), 2, "each step must be recorded in the trace");
    }

    #[test]
    fn multiply_decimal_rounds_half_up_without_floating_point_drift() {
        let context = ctx(&[("amount_cents", json!(10_001))]);
        let ops = json!([
            {"op":"multiply_decimal","from":{"reg":"amount_cents"},"factor":"0.75","into":"result"}
        ]).to_string();
        let (registers, _) = execute_operations(&ops, &context).unwrap();
        // 10001 * 0.75 = 7500.75 -> rounds to 7501
        assert_eq!(registers["result"], json!(7501));
    }

    #[test]
    fn cap_and_floor_bound_a_value() {
        let context = ctx(&[("v", json!(150))]);
        let ops = json!([
            {"op":"cap","value":{"reg":"v"},"max":100,"into":"capped"},
            {"op":"floor","value":{"reg":"capped"},"min":120,"into":"floored"}
        ]).to_string();
        let (registers, _) = execute_operations(&ops, &context).unwrap();
        assert_eq!(registers["capped"], json!(100));
        assert_eq!(registers["floored"], json!(120));
    }

    #[test]
    fn choose_picks_then_or_else_by_condition() {
        let context = ctx(&[("living", json!(true))]);
        let ops = json!([
            {"op":"choose","when":{"reg":"living"},"then":"living_track","else":"death_track","into":"result"}
        ]).to_string();
        let (registers, _) = execute_operations(&ops, &context).unwrap();
        assert_eq!(registers["result"], json!("living_track"));
    }

    #[test]
    fn require_input_fails_closed_when_field_missing_or_null() {
        let context = ctx(&[("present", json!("x")), ("explicit_null", Value::Null)]);
        assert!(execute_operations(&json!([{"op":"require_input","field":"present"}]).to_string(), &context).is_ok());
        assert!(execute_operations(&json!([{"op":"require_input","field":"missing_entirely"}]).to_string(), &context).is_err());
        assert!(execute_operations(&json!([{"op":"require_input","field":"explicit_null"}]).to_string(), &context).is_err());
    }

    #[test]
    fn unknown_register_reference_fails_closed() {
        let context = ctx(&[]);
        let ops = json!([{"op":"add_days","from":{"reg":"nope"},"days":1,"into":"result"}]).to_string();
        assert!(execute_operations(&ops, &context).is_err());
    }

    #[test]
    fn unknown_operator_fails_closed() {
        let context = ctx(&[]);
        let ops = json!([{"op":"eval_javascript","into":"result"}]).to_string();
        assert!(execute_operations(&ops, &context).is_err(), "the DSL must reject anything outside the fixed safe operator set");
    }
}
