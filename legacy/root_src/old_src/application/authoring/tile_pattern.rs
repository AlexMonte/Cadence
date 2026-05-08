//! Cadence tile script.
//!
//! This module owns a small, original text grammar for pattern structure inside
//! tiles. It lowers directly into Cadence host IR, so playback and preview stay
//! routed through the typed semantic layer.

#[derive(Debug, Clone, PartialEq)]
pub struct TileScript {
    root: TileScriptNode,
}

impl TileScript {
    #[must_use]
    pub fn new(root: TileScriptNode) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn root(&self) -> &TileScriptNode {
        &self.root
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TileScriptNode {
    Cue(CueToken),
    Rest,
    Sequence {
        mode: SequenceMode,
        children: Vec<TileScriptNode>,
    },
    Stack(Vec<TileScriptNode>),
    Chord(Vec<TileScriptNode>),
    Alternate {
        inner: Box<TileScriptNode>,
    },
    Elongate {
        inner: Box<TileScriptNode>,
        weight: u32,
    },
    Repeat {
        inner: Box<TileScriptNode>,
        times: u32,
    },
    Slow {
        inner: Box<TileScriptNode>,
        factor: ScriptFactor,
    },
    Fast {
        inner: Box<TileScriptNode>,
        factor: ScriptFactor,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceMode {
    Cycle,
    Chain,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CueToken(String);

impl CueToken {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CueToken {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScriptFactor(f64);

impl ScriptFactor {
    fn parse(raw: &str) -> Option<Self> {
        let value = raw.parse::<f64>().ok()?;
        (value.is_finite() && value > 0.0).then_some(Self(value))
    }

    #[must_use]
    pub fn value(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TileScriptError {
    #[error("tile script cannot be empty")]
    EmptyScript,
    #[error("unexpected end of input while parsing {context}")]
    UnexpectedEnd { context: &'static str },
    #[error("unexpected character `{found}` at byte {index}")]
    UnexpectedCharacter { found: char, index: usize },
    #[error("expected `{expected}` before byte {index}")]
    MissingDelimiter { expected: char, index: usize },
    #[error("branches cannot be empty inside {context}")]
    EmptyBranch { context: &'static str },
    #[error("expected a cue or group at byte {index}")]
    ExpectedTerm { index: usize },
    #[error("invalid numeric factor `{value}` at byte {index}")]
    InvalidFactor { value: String, index: usize },
    #[error("factor after `{operator}` must be greater than zero at byte {index}")]
    NonPositiveFactor {
        operator: &'static str,
        index: usize,
    },
    #[error("repeat count after `{operator}` must be at least one at byte {index}")]
    InvalidCount {
        operator: &'static str,
        index: usize,
    },
    #[error("cue `{cue}` is not valid in a {context}")]
    InvalidCueForContext { cue: String, context: &'static str },
}

#[cfg(test)]
pub(crate) fn parse(text: &str) -> Result<TileScript, TileScriptError> {
    let mut parser = Parser::new(text, ParseContext::Pattern);
    parser.parse()
}

pub(crate) fn parse_control_script(text: &str) -> Result<TileScript, TileScriptError> {
    parse_with_context(text, ParseContext::Control)
}

pub(crate) fn parse_note_script(text: &str) -> Result<TileScript, TileScriptError> {
    parse_with_context(text, ParseContext::Note)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseContext {
    #[cfg(test)]
    Pattern,
    Control,
    Note,
}

fn parse_with_context(text: &str, context: ParseContext) -> Result<TileScript, TileScriptError> {
    let mut parser = Parser::new(text, context);
    parser.parse()
}

struct Parser<'a> {
    input: &'a str,
    index: usize,
    context: ParseContext,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, context: ParseContext) -> Self {
        Self {
            input,
            index: 0,
            context,
        }
    }

    fn parse(&mut self) -> Result<TileScript, TileScriptError> {
        self.skip_whitespace();
        if self.peek().is_none() {
            return Err(TileScriptError::EmptyScript);
        }

        let root = self.parse_stack(None, SequenceMode::Cycle)?;
        self.skip_whitespace();

        if let Some(found) = self.peek() {
            return Err(TileScriptError::UnexpectedCharacter {
                found,
                index: self.index,
            });
        }

        Ok(TileScript::new(root))
    }

    fn parse_stack(
        &mut self,
        closing: Option<char>,
        mode: SequenceMode,
    ) -> Result<TileScriptNode, TileScriptError> {
        let mut branches: Vec<Vec<TileScriptNode>> = vec![Vec::new()];

        loop {
            self.skip_whitespace();

            match self.peek() {
                None => {
                    if let Some(expected) = closing {
                        return Err(TileScriptError::MissingDelimiter {
                            expected,
                            index: self.index,
                        });
                    }
                    break;
                }
                Some(found) if Some(found) == closing => {
                    self.bump();
                    break;
                }
                Some('|') => {
                    if branches.last().is_some_and(Vec::is_empty) {
                        return Err(TileScriptError::EmptyBranch {
                            context: context_name(closing),
                        });
                    }
                    self.bump();
                    branches.push(Vec::new());
                }
                Some(']') | Some('}') | Some('>') => {
                    return Err(TileScriptError::UnexpectedCharacter {
                        found: self.peek().expect("peeked above"),
                        index: self.index,
                    });
                }
                _ => {
                    let node = self.parse_item()?;
                    branches
                        .last_mut()
                        .expect("there is always at least one branch")
                        .push(node);
                }
            }
        }

        if branches.last().is_some_and(Vec::is_empty) {
            return Err(TileScriptError::EmptyBranch {
                context: context_name(closing),
            });
        }

        if branches.len() == 1 {
            return Ok(sequence_from(
                branches.pop().expect("one branch exists"),
                mode,
            ));
        }

        Ok(TileScriptNode::Stack(
            branches
                .into_iter()
                .map(|branch| sequence_from(branch, mode))
                .collect(),
        ))
    }

    fn parse_item(&mut self) -> Result<TileScriptNode, TileScriptError> {
        let first = self.parse_term()?;
        if self.context != ParseContext::Note {
            return Ok(first);
        }

        let mut children = vec![first];
        loop {
            let checkpoint = self.index;
            self.skip_whitespace();
            if self.peek() != Some(',') {
                self.index = checkpoint;
                break;
            }
            self.bump();
            self.skip_whitespace();
            children.push(self.parse_term()?);
        }

        if children.len() == 1 {
            Ok(children.pop().expect("single child exists"))
        } else {
            Ok(TileScriptNode::Chord(children))
        }
    }

    fn parse_term(&mut self) -> Result<TileScriptNode, TileScriptError> {
        self.skip_whitespace();

        let mut node = match self.peek() {
            Some('[') => {
                self.bump();
                self.parse_stack(Some(']'), SequenceMode::Cycle)?
            }
            Some('{') => {
                self.bump();
                self.parse_stack(Some('}'), SequenceMode::Chain)?
            }
            Some('<') => {
                self.bump();
                TileScriptNode::Alternate {
                    inner: Box::new(self.parse_stack(Some('>'), SequenceMode::Cycle)?),
                }
            }
            Some('.') | Some('_') => {
                self.bump();
                TileScriptNode::Rest
            }
            Some('"') => TileScriptNode::Cue(self.parse_quoted_cue()?),
            Some(']') | Some('}') | Some('>') | Some('|') | Some(',') => {
                return Err(TileScriptError::ExpectedTerm { index: self.index });
            }
            Some(_) => TileScriptNode::Cue(self.parse_cue()?),
            None => return Err(TileScriptError::UnexpectedEnd { context: "term" }),
        };

        loop {
            match self.peek() {
                Some('*') => {
                    self.bump();
                    let times = self.parse_positive_count("*")?;
                    node = TileScriptNode::Repeat {
                        inner: Box::new(node),
                        times,
                    };
                }
                Some('/') => {
                    self.bump();
                    let factor = self.parse_positive_factor("/")?;
                    node = TileScriptNode::Slow {
                        inner: Box::new(node),
                        factor,
                    };
                }
                Some('^') => {
                    self.bump();
                    let factor = self.parse_positive_factor("^")?;
                    node = TileScriptNode::Fast {
                        inner: Box::new(node),
                        factor,
                    };
                }
                Some('@') => {
                    self.bump();
                    let weight = self.parse_positive_count("@")?;
                    node = TileScriptNode::Elongate {
                        inner: Box::new(node),
                        weight,
                    };
                }
                _ => break,
            }
        }

        Ok(node)
    }

    fn parse_cue(&mut self) -> Result<CueToken, TileScriptError> {
        let start = self.index;

        while let Some(ch) = self.peek() {
            if self.context == ParseContext::Control
                && ch == '.'
                && self.input[start..self.index]
                    .chars()
                    .last()
                    .is_some_and(|prev| prev.is_ascii_digit())
                && self.peek_next().is_some_and(|next| next.is_ascii_digit())
            {
                self.bump();
                continue;
            }

            if ch.is_whitespace()
                || matches!(
                    ch,
                    '"' | '['
                        | ']'
                        | '{'
                        | '}'
                        | '<'
                        | '>'
                        | '|'
                        | ','
                        | '*'
                        | '/'
                        | '^'
                        | '@'
                        | '.'
                        | '_'
                )
            {
                break;
            }
            self.bump();
        }

        if start == self.index {
            return Err(TileScriptError::ExpectedTerm { index: self.index });
        }

        Ok(CueToken::new(&self.input[start..self.index]))
    }

    fn parse_quoted_cue(&mut self) -> Result<CueToken, TileScriptError> {
        let quote_index = self.index;
        let opening = self.bump();
        debug_assert_eq!(opening, Some('"'));

        let mut value = String::new();
        loop {
            match self.peek() {
                Some('"') => {
                    self.bump();
                    return Ok(CueToken::new(value));
                }
                Some('\\') => {
                    self.bump();
                    match self.peek() {
                        Some('"') | Some('\\') => {
                            value.push(self.bump().expect("escaped character should exist"));
                        }
                        Some(other) => {
                            value.push('\\');
                            value.push(other);
                            self.bump();
                        }
                        None => {
                            return Err(TileScriptError::MissingDelimiter {
                                expected: '"',
                                index: quote_index,
                            });
                        }
                    }
                }
                Some(ch) => {
                    value.push(ch);
                    self.bump();
                }
                None => {
                    return Err(TileScriptError::MissingDelimiter {
                        expected: '"',
                        index: quote_index,
                    });
                }
            }
        }
    }

    fn parse_positive_factor(
        &mut self,
        operator: &'static str,
    ) -> Result<ScriptFactor, TileScriptError> {
        let start = self.index;

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || ch == '.' {
                self.bump();
            } else {
                break;
            }
        }

        if start == self.index {
            return Err(TileScriptError::InvalidFactor {
                value: String::new(),
                index: start,
            });
        }

        let raw = &self.input[start..self.index];
        let Some(factor) = ScriptFactor::parse(raw) else {
            return Err(TileScriptError::InvalidFactor {
                value: raw.to_string(),
                index: start,
            });
        };

        if factor.value() <= 0.0 {
            return Err(TileScriptError::NonPositiveFactor {
                operator,
                index: start,
            });
        }

        Ok(factor)
    }

    fn parse_positive_count(&mut self, operator: &'static str) -> Result<u32, TileScriptError> {
        let start = self.index;

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.bump();
            } else {
                break;
            }
        }

        if self.peek() == Some('.') {
            return Err(TileScriptError::InvalidCount {
                operator,
                index: start,
            });
        }

        let raw = &self.input[start..self.index];
        let value = raw
            .parse::<u32>()
            .map_err(|_| TileScriptError::InvalidCount {
                operator,
                index: start,
            })?;

        if value == 0 {
            return Err(TileScriptError::InvalidCount {
                operator,
                index: start,
            });
        }

        Ok(value)
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.index..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        let current = self.peek()?;
        self.input[self.index + current.len_utf8()..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.index += ch.len_utf8();
        Some(ch)
    }
}

fn sequence_from(children: Vec<TileScriptNode>, mode: SequenceMode) -> TileScriptNode {
    if children.len() == 1 {
        children.into_iter().next().expect("one child exists")
    } else {
        TileScriptNode::Sequence { mode, children }
    }
}

fn context_name(closing: Option<char>) -> &'static str {
    match closing {
        Some(']') => "cycle group",
        Some('}') => "chain group",
        None => "script",
        Some(_) => "group",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cycle_slots_from_plain_whitespace_sequence() {
        let script = parse("bd hh cp").unwrap();

        assert_eq!(
            script.root(),
            &TileScriptNode::Sequence {
                mode: SequenceMode::Cycle,
                children: vec![
                    TileScriptNode::Cue(CueToken::from("bd")),
                    TileScriptNode::Cue(CueToken::from("hh")),
                    TileScriptNode::Cue(CueToken::from("cp")),
                ],
            }
        );
    }

    #[test]
    fn parses_quoted_cues_as_single_tokens() {
        let script = parse(r#"bd "/kick/@2/" cp"#).unwrap();

        assert_eq!(
            script.root(),
            &TileScriptNode::Sequence {
                mode: SequenceMode::Cycle,
                children: vec![
                    TileScriptNode::Cue(CueToken::from("bd")),
                    TileScriptNode::Cue(CueToken::from("/kick/@2/")),
                    TileScriptNode::Cue(CueToken::from("cp")),
                ],
            }
        );
    }

    #[test]
    fn parses_quoted_cues_with_escaped_quote_and_backslash() {
        let script = parse(r#""lead\\vox\"alt""#).unwrap();

        assert_eq!(
            script.root(),
            &TileScriptNode::Cue(CueToken::from(r#"lead\vox"alt"#))
        );
    }

    #[test]
    fn parses_decimal_cues_in_control_context() {
        let script = parse_control_script("[0.5 0.75 <1 0.75>]").unwrap();

        assert_eq!(
            script.root(),
            &TileScriptNode::Sequence {
                mode: SequenceMode::Cycle,
                children: vec![
                    TileScriptNode::Cue(CueToken::from("0.5")),
                    TileScriptNode::Cue(CueToken::from("0.75")),
                    TileScriptNode::Alternate {
                        inner: Box::new(TileScriptNode::Sequence {
                            mode: SequenceMode::Cycle,
                            children: vec![
                                TileScriptNode::Cue(CueToken::from("1")),
                                TileScriptNode::Cue(CueToken::from("0.75")),
                            ],
                        }),
                    },
                ],
            }
        );
    }

    #[test]
    fn parses_chain_and_stack_groups() {
        let script = parse("{bd hh} | [cp _]").unwrap();

        assert_eq!(
            script.root(),
            &TileScriptNode::Stack(vec![
                TileScriptNode::Sequence {
                    mode: SequenceMode::Chain,
                    children: vec![
                        TileScriptNode::Cue(CueToken::from("bd")),
                        TileScriptNode::Cue(CueToken::from("hh")),
                    ],
                },
                TileScriptNode::Sequence {
                    mode: SequenceMode::Cycle,
                    children: vec![
                        TileScriptNode::Cue(CueToken::from("cp")),
                        TileScriptNode::Rest,
                    ],
                },
            ])
        );
    }

    #[test]
    fn rejects_decimal_repeat_counts() {
        let error = parse("bd*2.5").unwrap_err();

        assert_eq!(
            error,
            TileScriptError::InvalidCount {
                operator: "*",
                index: 3,
            }
        );
    }

    #[test]
    fn parses_note_chord_groups_with_sequence_rests() {
        let script = parse_with_context("[e3,e7 e4 _ e7]", ParseContext::Note).unwrap();

        assert_eq!(
            script.root(),
            &TileScriptNode::Sequence {
                mode: SequenceMode::Cycle,
                children: vec![
                    TileScriptNode::Chord(vec![
                        TileScriptNode::Cue(CueToken::from("e3")),
                        TileScriptNode::Cue(CueToken::from("e7")),
                    ]),
                    TileScriptNode::Cue(CueToken::from("e4")),
                    TileScriptNode::Rest,
                    TileScriptNode::Cue(CueToken::from("e7")),
                ],
            }
        );
    }

    #[test]
    fn parses_alternation_and_elongation_in_pattern_context() {
        let script = parse_with_context("<bd cp@2>", ParseContext::Pattern).unwrap();

        assert_eq!(
            script.root(),
            &TileScriptNode::Alternate {
                inner: Box::new(TileScriptNode::Sequence {
                    mode: SequenceMode::Cycle,
                    children: vec![
                        TileScriptNode::Cue(CueToken::from("bd")),
                        TileScriptNode::Elongate {
                            inner: Box::new(TileScriptNode::Cue(CueToken::from("cp"))),
                            weight: 2,
                        },
                    ],
                }),
            }
        );
    }
}
