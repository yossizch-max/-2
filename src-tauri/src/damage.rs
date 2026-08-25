use crate::{
    error::{AppError, AppResult},
    models::{DamageInput, DamageResult},
};
use sha2::{Digest, Sha256};

const ALLOWED_KEYS: &[&str] = &[
    "past_wage_loss", "future_wage_loss", "pension_loss", "third_party_help",
    "medical_expenses", "travel_expenses", "pain_suffering",
    "dependency_loss", "estate_loss", "deductions"
];

pub fn calculate(regime: &str, life_state: &str, inputs: &[DamageInput]) -> AppResult<DamageResult> {
    if !matches!(regime, "pip" | "tort") {
        return Err(AppError::Validation("invalid damage regime".into()));
    }
    if !matches!(life_state, "living" | "death") {
        return Err(AppError::Validation("invalid life state".into()));
    }
    for input in inputs {
        if !ALLOWED_KEYS.contains(&input.key.as_str()) || input.cents < 0 {
            return Err(AppError::Validation(format!("invalid damage input {}", input.key)));
        }
    }
    if life_state == "death" && inputs.iter().any(|i| i.key == "future_wage_loss") {
        return Err(AppError::Validation(
            "death model cannot use living future-wage input".into()
        ));
    }

    let deductions = inputs.iter()
        .filter(|i| i.key == "deductions").map(|i| i.cents).sum::<i64>();
    let gross = inputs.iter()
        .filter(|i| i.key != "deductions").map(|i| i.cents).sum::<i64>();
    let net = gross.saturating_sub(deductions);

    let canonical = serde_json::to_vec(&(regime, life_state, inputs, gross, deductions, net))?;
    let integrity_sha256 = hex::encode(Sha256::digest(canonical));

    Ok(DamageResult {
        gross_cents: gross,
        deductions_cents: deductions,
        net_cents: net,
        integrity_sha256,
    })
}

/// Never trust a caller-supplied integrity hash for something about to become
/// immutable. This re-derives the result from the persisted inputs and refuses to
/// proceed if it doesn't match the totals already stored on the row - which would mean
/// the row was edited (or tampered with) out of band since it was last saved.
pub fn verify_for_lock(
    regime: &str, life_state: &str, inputs: &[DamageInput],
    stored_gross_cents: i64, stored_deductions_cents: i64, stored_net_cents: i64,
) -> AppResult<DamageResult> {
    let recomputed = calculate(regime, life_state, inputs)?;
    if (recomputed.gross_cents, recomputed.deductions_cents, recomputed.net_cents)
        != (stored_gross_cents, stored_deductions_cents, stored_net_cents)
    {
        return Err(AppError::Validation(
            "stored totals do not match a fresh recalculation from the persisted inputs - refusing to lock".into()
        ));
    }
    Ok(recomputed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sums_integer_cents_and_deductions() {
        let inputs = vec![
            DamageInput{key:"past_wage_loss".into(),cents:100_00,source:"manual".into()},
            DamageInput{key:"pain_suffering".into(),cents:50_00,source:"manual".into()},
            DamageInput{key:"deductions".into(),cents:20_00,source:"manual".into()},
        ];
        let r=calculate("pip","living",&inputs).unwrap();
        assert_eq!(r.gross_cents,150_00);
        assert_eq!(r.net_cents,130_00);
    }
    #[test]
    fn death_model_rejects_living_future_wage_key() {
        let inputs=vec![DamageInput{key:"future_wage_loss".into(),cents:1,source:"manual".into()}];
        assert!(calculate("tort","death",&inputs).is_err());
    }
    #[test]
    fn verify_for_lock_rejects_tampered_stored_totals() {
        let inputs=vec![DamageInput{key:"past_wage_loss".into(),cents:100_00,source:"manual".into()}];
        assert!(verify_for_lock("tort","living",&inputs,100_00,0,100_00).is_ok());
        let tampered=verify_for_lock("tort","living",&inputs,100_00,0,999_999);
        assert!(tampered.is_err(), "a stored net that doesn't match a fresh recompute from the same inputs must be rejected, not trusted");
    }
}
