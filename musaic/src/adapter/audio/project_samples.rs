//! The same ordered decoded sample bindings feed playback, audition, and export.
use crate::{
    application::session::MusaicProject,
    domain::project::samples::{SampleId, SampleManifest},
};
use cadence::infrastructure::playback::{ChokeGroup, SampleBank, SampleLoadOptions};
use std::collections::{BTreeMap, BTreeSet};

pub fn source_available(project: &MusaicProject, lead: SampleId) -> bool {
    project.samples.manifest().banks.get(&lead).map_or_else(
        || project.samples.decoded(lead).is_some(),
        |bank| {
            !bank.variants.is_empty()
                && bank
                    .variants
                    .iter()
                    .all(|variant| project.samples.decoded(variant.sample).is_some())
        },
    )
}

/// Replaces every project-owned alias as one complete ordered list. Individual
/// non-lead members retain their own direct names. A missing member disables its
/// lead alias, so a stale or shortened bank cannot play the wrong variant.
pub fn load_project_samples(project: &MusaicProject, bank: &SampleBank) -> BTreeSet<SampleId> {
    let mut loaded = BTreeSet::new();
    for (id, metadata) in &project.samples.manifest().samples {
        if !source_available(project, *id) {
            bank.remove(&id.runtime_name());
            continue;
        }
        let variants = if let Some(definition) = project.samples.manifest().banks.get(id) {
            definition
                .variants
                .iter()
                .map(|variant| {
                    let metadata = &project.samples.manifest().samples[&variant.sample];
                    (
                        project
                            .samples
                            .decoded(variant.sample)
                            .expect("checked decoded variant")
                            .clone(),
                        SampleLoadOptions {
                            root_pitch: metadata.options.root_pitch,
                            sustain_loop: metadata.options.sustain_loop.and_then(|region| {
                                cadence::adapter::sample_bank::SampleLoopRegion::new(
                                    region.start_frame,
                                    region.end_frame,
                                )
                            }),
                            bank: variant.bank.clone().map(Into::into),
                            pitch_range: variant.pitch_zone.as_ref().and_then(|zone| {
                                cadence::adapter::sample_bank::SamplePitchRange::new(
                                    zone.low, zone.high,
                                )
                            }),
                            playback_limit: variant
                                .playback_limit_ms
                                .map(|millis| std::time::Duration::from_millis(u64::from(millis))),
                            choke_group: variant.hat_choke.then_some(ChokeGroup::Hat),
                        },
                    )
                })
                .collect::<Vec<_>>()
        } else {
            vec![(
                project
                    .samples
                    .decoded(*id)
                    .expect("checked decoded sample")
                    .clone(),
                SampleLoadOptions {
                    root_pitch: metadata.options.root_pitch,
                    sustain_loop: metadata.options.sustain_loop.and_then(|region| {
                        cadence::adapter::sample_bank::SampleLoopRegion::new(
                            region.start_frame,
                            region.end_frame,
                        )
                    }),
                    ..Default::default()
                },
            )]
        };
        bank.replace_variants(id.runtime_name(), variants);
        loaded.insert(*id);
    }
    loaded
}

#[derive(Default)]
pub(super) struct ProjectSampleBindings {
    manifest: Option<SampleManifest>,
    decoded: BTreeMap<SampleId, usize>,
    names: BTreeSet<SampleId>,
}

impl ProjectSampleBindings {
    pub(super) fn synchronize(&mut self, project: &MusaicProject, bank: &SampleBank) {
        let decoded = project
            .samples
            .manifest()
            .samples
            .keys()
            .filter_map(|id| {
                project
                    .samples
                    .decoded(*id)
                    .map(|buffer| (*id, buffer.frames().as_ptr() as usize))
            })
            .collect::<BTreeMap<_, _>>();
        if self.manifest.as_ref() == Some(project.samples.manifest()) && self.decoded == decoded {
            return;
        }
        let names = load_project_samples(project, bank);
        for id in self.names.difference(&names) {
            bank.remove(&id.runtime_name());
        }
        self.names = names;
        self.manifest = Some(project.samples.manifest().clone());
        self.decoded = decoded;
    }
}
