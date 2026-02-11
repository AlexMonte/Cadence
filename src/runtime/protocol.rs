//! Typed runtime bridge protocol and revision-tracking resources.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const RUNTIME_PROTOCOL_VERSION: u16 = 1;

pub fn is_supported_protocol_version(version: u16) -> bool {
    version == RUNTIME_PROTOCOL_VERSION
}

#[derive(Message, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeIntent {
    Eval {
        protocol_version: u16,
        rev: u64,
        code: String,
    },
    Validate {
        protocol_version: u16,
        request_id: u64,
        code: String,
    },
    Play {
        protocol_version: u16,
    },
    Stop {
        protocol_version: u16,
    },
    SetTempo {
        protocol_version: u16,
        cpm: f32,
    },
}

impl RuntimeIntent {
    pub fn eval(rev: u64, code: impl Into<String>) -> Self {
        Self::Eval {
            protocol_version: RUNTIME_PROTOCOL_VERSION,
            rev,
            code: code.into(),
        }
    }

    pub fn play() -> Self {
        Self::Play {
            protocol_version: RUNTIME_PROTOCOL_VERSION,
        }
    }

    pub fn validate(request_id: u64, code: impl Into<String>) -> Self {
        Self::Validate {
            protocol_version: RUNTIME_PROTOCOL_VERSION,
            request_id,
            code: code.into(),
        }
    }

    pub fn stop() -> Self {
        Self::Stop {
            protocol_version: RUNTIME_PROTOCOL_VERSION,
        }
    }

    pub fn set_tempo(cpm: f32) -> Self {
        Self::SetTempo {
            protocol_version: RUNTIME_PROTOCOL_VERSION,
            cpm,
        }
    }

    pub fn protocol_version(&self) -> u16 {
        match self {
            Self::Eval {
                protocol_version, ..
            }
            | Self::Validate {
                protocol_version, ..
            }
            | Self::Play { protocol_version }
            | Self::Stop { protocol_version }
            | Self::SetTempo {
                protocol_version, ..
            } => *protocol_version,
        }
    }
}

#[derive(Message, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeEvent {
    Ready {
        protocol_version: u16,
    },
    Status {
        protocol_version: u16,
        playing: bool,
        current_rev: u64,
    },
    Error {
        protocol_version: u16,
        rev: u64,
        message: String,
    },
    Validation {
        protocol_version: u16,
        request_id: u64,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        column: Option<u32>,
    },
}

impl RuntimeEvent {
    pub fn ready() -> Self {
        Self::Ready {
            protocol_version: RUNTIME_PROTOCOL_VERSION,
        }
    }

    pub fn status(playing: bool, current_rev: u64) -> Self {
        Self::Status {
            protocol_version: RUNTIME_PROTOCOL_VERSION,
            playing,
            current_rev,
        }
    }

    pub fn error(rev: u64, message: impl Into<String>) -> Self {
        Self::Error {
            protocol_version: RUNTIME_PROTOCOL_VERSION,
            rev,
            message: message.into(),
        }
    }

    pub fn validation_ok(request_id: u64) -> Self {
        Self::Validation {
            protocol_version: RUNTIME_PROTOCOL_VERSION,
            request_id,
            ok: true,
            message: None,
            line: None,
            column: None,
        }
    }

    pub fn validation_error(
        request_id: u64,
        message: impl Into<String>,
        line: Option<u32>,
        column: Option<u32>,
    ) -> Self {
        Self::Validation {
            protocol_version: RUNTIME_PROTOCOL_VERSION,
            request_id,
            ok: false,
            message: Some(message.into()),
            line,
            column,
        }
    }

    pub fn protocol_version(&self) -> u16 {
        match self {
            Self::Ready { protocol_version }
            | Self::Status {
                protocol_version, ..
            }
            | Self::Error {
                protocol_version, ..
            }
            | Self::Validation {
                protocol_version, ..
            } => *protocol_version,
        }
    }
}

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeRevisions {
    pub draft_rev: u64,
    pub commit_rev: u64,
    pub runtime_rev: u64,
}

impl RuntimeRevisions {
    pub fn bump_draft(&mut self) -> u64 {
        self.draft_rev += 1;
        self.draft_rev
    }

    pub fn commit_from_draft(&mut self) -> u64 {
        self.commit_rev = self.commit_rev.max(self.draft_rev);
        self.commit_rev
    }

    pub fn commit_next(&mut self) -> u64 {
        self.commit_rev += 1;
        self.commit_rev
    }

    pub fn mark_runtime_rev(&mut self, rev: u64) {
        self.runtime_rev = self.runtime_rev.max(rev);
    }
}

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct RuntimeState {
    pub ready: bool,
    pub playing: bool,
    pub tempo_cpm: f32,
    pub last_eval_rev: u64,
    pub last_ok_rev: Option<u64>,
    pub last_error_rev: Option<u64>,
    pub last_error: Option<String>,
    pub last_code: String,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            ready: false,
            playing: false,
            tempo_cpm: 120.0,
            last_eval_rev: 0,
            last_ok_rev: None,
            last_error_rev: None,
            last_error: None,
            last_code: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_helpers_default_to_v1() {
        let eval = RuntimeIntent::eval(7, "s(\"bd\")");
        let play = RuntimeIntent::play();
        let stop = RuntimeIntent::stop();
        let tempo = RuntimeIntent::set_tempo(120.0);
        let ready = RuntimeEvent::ready();
        let status = RuntimeEvent::status(true, 7);
        let error = RuntimeEvent::error(7, "boom");

        assert_eq!(eval.protocol_version(), RUNTIME_PROTOCOL_VERSION);
        assert_eq!(play.protocol_version(), RUNTIME_PROTOCOL_VERSION);
        assert_eq!(stop.protocol_version(), RUNTIME_PROTOCOL_VERSION);
        assert_eq!(tempo.protocol_version(), RUNTIME_PROTOCOL_VERSION);
        assert_eq!(ready.protocol_version(), RUNTIME_PROTOCOL_VERSION);
        assert_eq!(status.protocol_version(), RUNTIME_PROTOCOL_VERSION);
        assert_eq!(error.protocol_version(), RUNTIME_PROTOCOL_VERSION);
        assert!(is_supported_protocol_version(RUNTIME_PROTOCOL_VERSION));
        assert!(!is_supported_protocol_version(9999));
    }

    #[test]
    fn serde_wire_format_uses_tagged_type_field() {
        let intent = RuntimeIntent::eval(4, "s(\"bd\")");
        let encoded = serde_json::to_value(intent).expect("intent should serialize");
        assert_eq!(encoded["type"], serde_json::json!("eval"));
        assert_eq!(encoded["protocol_version"], serde_json::json!(1));
        assert_eq!(encoded["rev"], serde_json::json!(4));

        let event = RuntimeEvent::status(true, 4);
        let encoded_event = serde_json::to_value(event).expect("event should serialize");
        assert_eq!(encoded_event["type"], serde_json::json!("status"));
        assert_eq!(encoded_event["current_rev"], serde_json::json!(4));
    }

    #[test]
    fn runtime_revisions_are_monotonic_for_runtime() {
        let mut revs = RuntimeRevisions::default();
        assert_eq!(revs.bump_draft(), 1);
        assert_eq!(revs.bump_draft(), 2);
        assert_eq!(revs.commit_from_draft(), 2);
        revs.mark_runtime_rev(1);
        assert_eq!(revs.runtime_rev, 1);
        revs.mark_runtime_rev(0);
        assert_eq!(revs.runtime_rev, 1);
        revs.mark_runtime_rev(10);
        assert_eq!(revs.runtime_rev, 10);
    }

    #[test]
    fn commit_from_draft_never_regresses_commit_rev() {
        let mut revs = RuntimeRevisions {
            draft_rev: 1,
            commit_rev: 5,
            runtime_rev: 5,
        };

        assert_eq!(revs.commit_from_draft(), 5);
        assert_eq!(revs.commit_rev, 5);
    }
}
