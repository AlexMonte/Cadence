//! Host labels for Tessera's authored flow nodes. The complete language node is
//! retained in the document, including imported signatures and named members.
use tessera::prelude::{FlowControlKind as K, FlowControlNode, FlowControlPolicy as P};

pub const KINDS: [K; 8] = [
    K::Layer,
    K::Merge,
    K::Mix,
    K::Split,
    K::Mask,
    K::Switch,
    K::Route,
    K::Choice,
];
pub fn label(kind: K) -> &'static str {
    match kind {
        K::Layer => "Layer",
        K::Merge => "Merge",
        K::Mix => "Mix",
        K::Split => "Split",
        K::Mask => "Mask",
        K::Switch => "Switch",
        K::Route => "Route",
        K::Choice => "Choice",
    }
}
pub fn policy_label(policy: &P) -> String {
    match policy {
        P::Layer => "Play together".into(),
        P::MergeAppend => "Append streams".into(),
        P::MergeInterleave => "Interleave events".into(),
        P::MergePriority => "Prioritize first stream".into(),
        P::MergeDeduplicate => "Remove duplicate events".into(),
        P::MixFieldBlend => "Blend event fields".into(),
        P::MixWeighted => "Weighted blend".into(),
        P::MixGainAverage => "Average gains".into(),
        P::SplitCopyToAll => "Copy to every branch".into(),
        P::SplitByIndexModulo => "Distribute events in order".into(),
        P::SplitByEventField => "Split by event field".into(),
        P::SplitByPitchRange { threshold_octave } => format!("Split at octave {threshold_octave}"),
        P::MaskGate => "Gate by mask".into(),
        P::MaskScale => "Scale by mask".into(),
        P::MaskClip => "Clip to mask".into(),
        P::MaskInvertGate => "Invert mask gate".into(),
        P::SwitchCycleIndex => "Switch each cycle".into(),
        P::SwitchControlValue => "Switch by control value".into(),
        P::SwitchSeededRandom => "Repeatable random switch".into(),
        P::RouteByEventField => "Route by event field".into(),
        P::RouteByIndexModulo => "Route events in order".into(),
        P::RouteByControlValue => "Route by control value".into(),
        P::RouteByLabel => "Route by label".into(),
        P::ChoiceCycle => "Choose each cycle".into(),
        P::ChoiceSeededRandom => "Repeatable random choice".into(),
        P::ChoiceWeighted => "Weighted choice".into(),
    }
}
pub fn description(control: &FlowControlNode) -> String {
    let mut lines = vec![policy_label(&control.policy)];
    for socket in &control.signature.input_sockets {
        let shape = match socket.shape {
            tessera::prelude::StreamShape::ScalarPattern => "value",
            tessera::prelude::StreamShape::ControlPattern => "control",
            tessera::prelude::StreamShape::NotePattern => "notes",
            _ => "pattern",
        };
        lines.push(format!("In: {} ({shape})", socket.port.0));
    }
    for (group, members) in &control.members.inputs {
        lines.push(format!(
            "In {}: {}",
            group.0,
            members
                .iter()
                .map(|m| m.0.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    for socket in &control.signature.output_sockets {
        lines.push(format!("Out: {}", socket.port.0));
    }
    for (group, members) in &control.members.outputs {
        lines.push(format!(
            "Out {}: {}",
            group.0,
            members
                .iter()
                .map(|m| m.0.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    lines.join("\n")
}

pub fn policies(control: &FlowControlNode) -> Vec<P> {
    match control.kind {
        K::Layer => vec![P::Layer],
        K::Merge => vec![
            P::MergeAppend,
            P::MergeInterleave,
            P::MergePriority,
            P::MergeDeduplicate,
        ],
        K::Mix => vec![P::MixFieldBlend, P::MixWeighted, P::MixGainAverage],
        K::Split => vec![
            P::SplitByIndexModulo,
            P::SplitCopyToAll,
            P::SplitByEventField,
            P::SplitByPitchRange {
                threshold_octave: match control.policy {
                    P::SplitByPitchRange { threshold_octave } => threshold_octave,
                    _ => 4,
                },
            },
        ],
        K::Mask => vec![P::MaskGate, P::MaskScale, P::MaskClip, P::MaskInvertGate],
        K::Switch => vec![
            P::SwitchCycleIndex,
            P::SwitchControlValue,
            P::SwitchSeededRandom,
        ],
        K::Route => vec![
            P::RouteByIndexModulo,
            P::RouteByEventField,
            P::RouteByControlValue,
            P::RouteByLabel,
        ],
        K::Choice => vec![P::ChoiceCycle, P::ChoiceSeededRandom, P::ChoiceWeighted],
    }
}

pub fn inputs(control: &FlowControlNode) -> Vec<tessera::prelude::InputEndpoint> {
    use tessera::prelude::InputEndpoint as E;
    control
        .signature
        .input_sockets
        .iter()
        .map(|s| E::Socket(s.port.clone()))
        .chain(control.members.inputs.iter().flat_map(|(g, ms)| {
            ms.iter().map(|m| E::GroupMember {
                group: g.clone(),
                member: m.clone(),
            })
        }))
        .collect()
}
pub fn outputs(control: &FlowControlNode) -> Vec<tessera::prelude::OutputEndpoint> {
    use tessera::prelude::OutputEndpoint as E;
    control
        .signature
        .output_sockets
        .iter()
        .map(|s| E::Socket(s.port.clone()))
        .chain(control.members.outputs.iter().flat_map(|(g, ms)| {
            ms.iter().map(|m| E::GroupMember {
                group: g.clone(),
                member: m.clone(),
            })
        }))
        .collect()
}

pub fn port_config(
    bindings: &tessera::prelude::NodeSpatialBindings,
) -> super::document::PortEndpointConfig {
    super::document::port_config(bindings)
}
