//! Paint-ready Sound tile state, derived without decoding audio in UI.

use tessera::prelude::NodeId;

use crate::{
    application::session::MusaicProject,
    domain::{
        document::{DocumentNodeKind, DocumentQueries},
        instrument::{InstrumentDefinition, InstrumentSource},
        project::samples::SampleId,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleChoicePaint {
    pub id: SampleId,
    pub label: String,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SampleOptionsPaint {
    pub sample: SampleId,
    pub frame_count: u64,
    pub options: crate::domain::project::samples::SampleImportOptions,
    pub bank: Option<crate::domain::project::samples::SampleBankDefinition>,
    pub available_members: Vec<SampleChoicePaint>,
    pub member_of: Option<String>,
}
// Metadata validation rejects non-finite values before authoring/import.
impl Eq for SampleOptionsPaint {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundPaint {
    pub sample_options: Option<SampleOptionsPaint>,
    pub instrument: InstrumentDefinition,
    pub presets: Vec<(String, InstrumentDefinition)>,
    pub samples: Vec<SampleChoicePaint>,
    pub details: String,
    pub waveform: Vec<u16>,
    pub diagnostic: Option<String>,
}

pub fn sound_paint(project: &MusaicProject, node: &NodeId) -> Option<SoundPaint> {
    let Some(DocumentNodeKind::Sound(sound)) =
        DocumentQueries::new(&project.document).node_kind(node)
    else {
        return None;
    };
    let instrument = sound.definition.clone();
    let samples = project
        .samples
        .manifest()
        .samples
        .iter()
        .map(|(id, sample)| SampleChoicePaint {
            id: *id,
            label: sample.source_name.clone(),
            available: project.samples.decoded(*id).is_some(),
        })
        .collect();
    let (details, waveform, diagnostic) = match &instrument.source {
        InstrumentSource::Sample(id) => {
            let details = project
                .samples
                .manifest()
                .samples
                .get(id)
                .map(|sample| {
                    let channels = if sample.channels == 1 {
                        "mono"
                    } else {
                        "stereo"
                    };
                    format!(
                        "{:.2}s · {} Hz · {channels}",
                        sample.frame_count as f64 / sample.sample_rate as f64,
                        sample.sample_rate
                    )
                })
                .unwrap_or_else(|| "Sample is absent from the project manifest".into());
            let diagnostic = project
                .samples
                .diagnostics()
                .iter()
                .find(|d| d.sample == *id)
                .map(|d| d.message.clone());
            (
                details,
                project
                    .samples
                    .waveform_peaks(*id)
                    .unwrap_or_default()
                    .to_vec(),
                diagnostic,
            )
        }
        InstrumentSource::Preset(preset) => (
            match preset {
                crate::domain::instrument::SynthPreset::Bass => "Bass · saw · low-pass 900 Hz",
                crate::domain::instrument::SynthPreset::Pad => "Pad · triangle · low-pass 3500 Hz",
                crate::domain::instrument::SynthPreset::Percussion => {
                    "Percussion · noise · high-pass 1800 Hz · low-pass 9000 Hz"
                }
            }
            .into(),
            Vec::new(),
            None,
        ),
        InstrumentSource::Synth(_) => (
            "Built-in synthesizer · follows note pitch".into(),
            Vec::new(),
            None,
        ),
        InstrumentSource::Kit => (
            "Built-in drum kit · each named hit selects its one-shot".into(),
            Vec::new(),
            None,
        ),
        InstrumentSource::Drum(_) => (
            "Built-in percussion · one-shot sample".into(),
            Vec::new(),
            None,
        ),
    };
    let sample_options = match instrument.source {
        InstrumentSource::Sample(id) => {
            project
                .samples
                .manifest()
                .samples
                .get(&id)
                .map(|metadata| SampleOptionsPaint {
                    sample: id,
                    frame_count: metadata.frame_count,
                    options: metadata.options.clone(),
                    bank: project.samples.manifest().banks.get(&id).cloned(),
                    member_of: project
                        .samples
                        .manifest()
                        .banks
                        .iter()
                        .find(|(lead, bank)| {
                            **lead != id && bank.variants.iter().any(|variant| variant.sample == id)
                        })
                        .map(|(lead, _)| {
                            project.samples.manifest().samples[lead].source_name.clone()
                        }),
                    available_members: project
                        .samples
                        .manifest()
                        .samples
                        .iter()
                        .filter(|(member, _)| {
                            !project.samples.manifest().banks.iter().any(|(lead, bank)| {
                                *lead != id
                                    && bank
                                        .variants
                                        .iter()
                                        .any(|variant| variant.sample == **member)
                            })
                        })
                        .map(|(member, metadata)| SampleChoicePaint {
                            id: *member,
                            label: metadata.source_name.clone(),
                            available: project.samples.decoded(*member).is_some(),
                        })
                        .collect(),
                })
        }
        _ => None,
    };
    Some(SoundPaint {
        sample_options,
        presets: project
            .sound_library
            .iter()
            .map(|(name, sound)| (name.clone(), sound.clone()))
            .collect(),
        instrument,
        samples,
        details,
        waveform,
        diagnostic,
    })
}
