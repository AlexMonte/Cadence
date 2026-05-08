use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptInputContext {
    SourcePattern,
    NotePattern,
    StructuralPattern,
    Pattern,
}

impl ScriptInputContext {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::SourcePattern => "source pattern",
            Self::NotePattern => "note pattern",
            Self::StructuralPattern => "structural pattern",
            Self::Pattern => "pattern",
        }
    }

    #[must_use]
    pub fn expects_text_reason(self, param_id: &str) -> String {
        format!("`{param_id}` expects text when building a {}", self.label())
    }

    #[must_use]
    pub fn invalid_script_reason(self, error: impl std::fmt::Display) -> String {
        format!("invalid {}: {error}", self.label())
    }

    #[must_use]
    pub fn for_piece_param(
        piece_id: &str,
        param_id: &str,
        text_semantics: Option<&str>,
    ) -> Option<Self> {
        match (piece_id, param_id) {
            ("cadence.sound", "value") => Some(Self::SourcePattern),
            ("cadence.note", "value") => Some(Self::NotePattern),
            ("cadence.mask", "by") => Some(Self::StructuralPattern),
            _ => match text_semantics?.trim() {
                "cadence_sample_script" => Some(Self::SourcePattern),
                "cadence_note_script" => Some(Self::NotePattern),
                "cadence_pattern_script" => Some(Self::Pattern),
                _ => None,
            },
        }
    }
}

#[must_use]
pub fn parse_pitch_text(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(number) = trimmed.parse::<f64>() {
        return number.is_finite().then_some(number);
    }

    let mut chars = trimmed.chars();
    let head = chars.next()?.to_ascii_lowercase();
    let base = match head {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };

    let suffix = chars.as_str();
    let suffix = suffix.trim();
    let (accidental, octave_text) = if let Some(rest) = suffix
        .strip_prefix('s')
        .or_else(|| suffix.strip_prefix('S'))
    {
        (1_i32, rest)
    } else {
        let accidental_len = suffix
            .chars()
            .take_while(|ch| matches!(ch, '#' | 'b' | 'B'))
            .count();
        let (accidentals, octave_text) = suffix.split_at(accidental_len);
        let accidental = accidentals.chars().fold(0_i32, |sum, ch| match ch {
            '#' => sum + 1,
            'b' | 'B' => sum - 1,
            _ => sum,
        });
        (accidental, octave_text)
    };
    let octave = if octave_text.trim().is_empty() {
        4
    } else {
        octave_text.trim().parse::<i32>().ok()?
    };

    Some(((octave + 1) * 12 + base + accidental) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piece_param_context_overrides_generic_text_semantics() {
        assert_eq!(
            ScriptInputContext::for_piece_param(
                "cadence.mask",
                "by",
                Some("cadence_pattern_script")
            ),
            Some(ScriptInputContext::StructuralPattern)
        );
        assert_eq!(
            ScriptInputContext::for_piece_param(
                "cadence.sound",
                "value",
                Some("cadence_sample_script")
            ),
            Some(ScriptInputContext::SourcePattern)
        );
        assert_eq!(
            ScriptInputContext::for_piece_param(
                "cadence.output",
                "pattern",
                Some("cadence_pattern_script")
            ),
            Some(ScriptInputContext::Pattern)
        );
    }

    #[test]
    fn parse_pitch_text_accepts_named_and_numeric_pitches() {
        assert_eq!(parse_pitch_text("e3"), Some(52.0));
        assert_eq!(parse_pitch_text("f#5"), Some(78.0));
        assert_eq!(parse_pitch_text("60"), Some(60.0));
        assert_eq!(parse_pitch_text("bd"), None);
        assert_eq!(parse_pitch_text("NaN"), None);
        assert_eq!(parse_pitch_text("inf"), None);
    }
}
