//! Compact authoring that expands into ordinary Tessera containers and owned groups.
//! Sound names stay symbolic until the host chooses an instrument.
use crate::domain::{AtomModifier, ContainerKind, Rational};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniPattern {
    pub kind: MiniPatternKind,
    pub modifiers: Vec<AtomModifier>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MiniPatternKind {
    Value(String),
    Rest,
    Group(ContainerKind, Vec<MiniPattern>),
}
impl MiniPattern {
    fn new(kind: MiniPatternKind) -> Self {
        Self {
            kind,
            modifiers: Vec::new(),
        }
    }
}

/// Parse sequences, subdivisions, layers, alternation, rests, repetition, weight,
/// and constant speed changes. No JavaScript is executed. Offsets are byte offsets.
/// Limits bound input size, nesting, and authored expressions before expansion.
pub fn parse_mini_notation(text: &str) -> Result<MiniPattern, String> {
    if text.len() > 32_768 {
        return Err("Pattern exceeds 32768 bytes.".into());
    }
    let mut parser = Parser {
        text,
        at: 0,
        nodes: 0,
    };
    let result = parser.group(None, ContainerKind::Sequence, 0)?;
    parser.space();
    if parser.at != text.len() {
        return parser.error("Unexpected closing delimiter");
    }
    expansion_cost(&result)?;
    timing_cost(&result)?;
    Ok(result)
}
struct Parser<'a> {
    text: &'a str,
    at: usize,
    nodes: usize,
}
impl Parser<'_> {
    fn error<T>(&self, message: &str) -> Result<T, String> {
        Err(format!("{message} at byte {}.", self.at))
    }
    fn peek(&self) -> Option<char> {
        self.text[self.at..].chars().next()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += c.len_utf8();
        Some(c)
    }
    fn space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }
    fn group(
        &mut self,
        end: Option<char>,
        kind: ContainerKind,
        depth: usize,
    ) -> Result<MiniPattern, String> {
        if depth > 64 {
            return self.error("Pattern nesting exceeds 64 levels");
        }
        let mut branches = Vec::new();
        let mut sequence = Vec::new();
        loop {
            self.space();
            match self.peek() {
                None | Some(']' | '>') => {
                    if self.peek() != end {
                        return self.error("Unmatched pattern delimiter");
                    }
                    if end.is_some() {
                        self.bump();
                    }
                    break;
                }
                Some(',') => {
                    if sequence.is_empty() {
                        return self.error("Layer needs a pattern before the comma");
                    }
                    self.bump();
                    branches.push(MiniPattern::new(MiniPatternKind::Group(
                        kind,
                        std::mem::take(&mut sequence),
                    )));
                }
                _ => sequence.push(self.item(depth + 1)?),
            }
        }
        if sequence.is_empty() {
            return self.error("Expected a pattern");
        }
        let last = MiniPattern::new(MiniPatternKind::Group(kind, sequence));
        if branches.is_empty() {
            Ok(last)
        } else {
            branches.push(last);
            Ok(MiniPattern::new(MiniPatternKind::Group(
                ContainerKind::Layer,
                branches,
            )))
        }
    }
    fn item(&mut self, depth: usize) -> Result<MiniPattern, String> {
        self.nodes += 1;
        if self.nodes > 4096 {
            return self.error("Pattern exceeds 4096 expressions");
        }
        let mut item = match self.peek() {
            Some('[') => {
                self.bump();
                self.group(Some(']'), ContainerKind::Sequence, depth)?
            }
            Some('<') => {
                self.bump();
                self.group(Some('>'), ContainerKind::Alternate, depth)?
            }
            _ => {
                let start = self.at;
                while self
                    .peek()
                    .is_some_and(|c| !c.is_whitespace() && !"[],<>!* /@".contains(c))
                {
                    self.bump();
                }
                let value = &self.text[start..self.at];
                if value.is_empty() {
                    return self.error("Expected a value or group");
                }
                // Reject unsupported grammar instead of treating it as a sound name.
                if value
                    .chars()
                    .any(|c| !c.is_alphanumeric() && !"_:#.-+♯♭~".contains(c))
                {
                    return self.error("Unsupported pattern token");
                }
                MiniPattern::new(if value == "~" || value == "-" {
                    MiniPatternKind::Rest
                } else {
                    MiniPatternKind::Value(value.into())
                })
            }
        };
        while let Some(op @ ('!' | '*' | '/' | '@')) = self.peek() {
            self.bump();
            let start = self.at;
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
                self.bump();
            }
            let value = parse_pattern_number(&self.text[start..self.at])
                .map_err(|e| format!("{e} at byte {start}."))?;
            if value <= Rational::zero() || value > Rational::from_integer(1024) {
                return self.error("Pattern factor must be greater than zero and at most 1024");
            }
            item.modifiers.push(match op {
                '!' if value.denominator == 1 => AtomModifier::Replicate(value.numerator as u32),
                '!' => return self.error("Repeat count must be a whole number"),
                '*' => AtomModifier::Fast(value),
                '/' => AtomModifier::Slow(value),
                '@' => AtomModifier::Elongate(value),
                _ => unreachable!(),
            });
        }
        Ok(item)
    }
}

/// Exact finite decimal parsing for authored controls; no floating-point rounding.
pub fn parse_pattern_number(text: &str) -> Result<Rational, String> {
    let text = text.trim();
    let (negative, digits) = if let Some(t) = text.strip_prefix('-') {
        (true, t)
    } else {
        (false, text.strip_prefix('+').unwrap_or(text))
    };
    let mut parts = digits.split('.');
    let whole = parts.next().unwrap_or("");
    let fractional = parts.next().unwrap_or("");
    if parts.next().is_some()
        || whole.len() + fractional.len() == 0
        || whole.len() + fractional.len() > 12
        || !whole
            .bytes()
            .chain(fractional.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return Err("Expected a decimal number with at most 12 digits".into());
    }
    let denominator = 10i64.pow(fractional.len() as u32);
    let number: i64 = format!("{whole}{fractional}")
        .parse()
        .map_err(|_| "Invalid number")?;
    Ok(Rational::new(
        if negative { -number } else { number },
        denominator,
    ))
}

// A conservative bound for the existing i64 rational clock. Reject combinations
// whose intermediate subdivisions/speed/offset arithmetic could exceed it.
pub(super) fn timing_cost(pattern: &MiniPattern) -> Result<u64, String> {
    let base = match &pattern.kind {
        MiniPatternKind::Group(_, children) => {
            let child = children
                .iter()
                .map(timing_cost)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .max()
                .unwrap_or(1);
            child
                .checked_mul(children.len() as u64)
                .ok_or("Pattern timing is too complex")?
        }
        _ => 1,
    };
    modifier_cost(base, &pattern.modifiers)
}
pub(super) fn modifier_cost(mut cost: u64, modifiers: &[AtomModifier]) -> Result<u64, String> {
    for modifier in modifiers {
        let factor = match modifier {
            AtomModifier::Fast(v)
            | AtomModifier::Slow(v)
            | AtomModifier::Elongate(v)
            | AtomModifier::Late(v) => v.numerator.unsigned_abs().max(v.denominator as u64).max(1),
            AtomModifier::Replicate(n) => u64::from(*n),
            _ => 1,
        };
        cost = cost
            .checked_mul(factor)
            .ok_or("Pattern timing is too complex")?;
        if cost > 16_777_216 {
            return Err("Pattern timing is too complex; reduce nested subdivisions, repeats or timing controls.".into());
        }
    }
    Ok(cost)
}

fn expansion_cost(pattern: &MiniPattern) -> Result<usize, String> {
    let mut count = match &pattern.kind {
        MiniPatternKind::Group(_, children) => children
            .iter()
            .map(expansion_cost)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum::<usize>(),
        _ => 1,
    };
    for modifier in &pattern.modifiers {
        if let AtomModifier::Replicate(n) = modifier {
            count = count
                .checked_mul(*n as usize)
                .ok_or("Too many repeated steps")?;
        }
        if count > 16384 {
            return Err("Pattern expands beyond 16384 steps.".into());
        }
    }
    if count > 16384 {
        return Err("Pattern expands beyond 16384 steps.".into());
    }
    Ok(count)
}
