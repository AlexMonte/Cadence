//! Shared tempo parsing used by project editing, preview, and runtime playback.

use crate::adapter::cadence_core::PlaybackTime;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTempoExpr {
    pub raw: String,
    pub cps: PlaybackTime,
}

pub fn parse_cps_expr(expr: Option<&str>) -> Result<Option<ParsedTempoExpr>, String> {
    let expr = expr.unwrap_or_default().trim();
    if expr.is_empty() {
        return Ok(None);
    }

    let mut terms = expr.split('/').map(str::trim);
    let first = parse_positive_finite_term(terms.next().unwrap_or_default(), 1)?;
    let value = terms
        .enumerate()
        .try_fold(first, |current, (index, term)| {
            let divisor = parse_positive_finite_term(term, index + 2)?;
            Ok::<_, String>(current / divisor)
        })?;

    if !value.is_finite() || value <= 0.0 {
        return Err("tempo expression must resolve to a positive finite value".to_string());
    }

    Ok(Some(ParsedTempoExpr {
        raw: expr.to_string(),
        cps: float_to_time(value),
    }))
}

fn parse_positive_finite_term(term: &str, position: usize) -> Result<f64, String> {
    let trimmed = term.trim();
    if trimmed.is_empty() {
        return Err(format!("tempo term {position} is empty"));
    }

    let value = trimmed
        .parse::<f64>()
        .map_err(|_| format!("tempo term {position} is not a number"))?;
    if !value.is_finite() {
        return Err(format!("tempo term {position} must be finite"));
    }
    if value <= 0.0 {
        return Err(format!("tempo term {position} must be greater than zero"));
    }
    Ok(value)
}

fn float_to_time(value: f64) -> PlaybackTime {
    const SCALE: i64 = 1_000;
    PlaybackTime::new((value * SCALE as f64).round() as i64, SCALE)
}

#[cfg(test)]
mod tests {
    use super::parse_cps_expr;

    #[test]
    fn parses_left_associative_tempo_chains() {
        let parsed = parse_cps_expr(Some("113/60/4"))
            .expect("tempo should parse")
            .expect("tempo present");
        assert_eq!(parsed.cps.numerator(), 471);
        assert_eq!(parsed.cps.denominator(), 1_000);
    }

    #[test]
    fn rejects_invalid_tempo_terms() {
        for expr in ["0", "-1", "120/0", "NaN", "1/inf", "120//60"] {
            assert!(
                parse_cps_expr(Some(expr)).is_err(),
                "expected tempo parse failure for {expr}"
            );
        }
    }
}
