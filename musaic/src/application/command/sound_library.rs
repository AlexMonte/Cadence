//! Reusable sound-library edits remain in the same project command authority.
use crate::{
    application::session::MusaicProject,
    domain::{
        document::{DocumentNodeKind, DocumentQueries},
        instrument::{SoundLibrary, sound_library_name, validate_sound_library},
    },
};
use serde::{Deserialize, Serialize};
use tessera::prelude::NodeId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SoundLibraryCommand {
    Save { sound: NodeId, name: String },
    Rename { name: String, new_name: String },
    Delete { name: String },
}

pub fn apply(
    project: &mut MusaicProject,
    command: &SoundLibraryCommand,
) -> Result<Option<SoundLibrary>, String> {
    let mut next = project.sound_library.clone();
    match command {
        SoundLibraryCommand::Save { sound, name } => {
            if !matches!(
                DocumentQueries::new(&project.document).node_kind(sound),
                Some(DocumentNodeKind::Sound(_))
            ) {
                return Err("Choose a Sound tile to save its design".into());
            }
            let name = sound_library_name(name)?;
            if next
                .keys()
                .any(|existing| existing.to_lowercase() == name.to_lowercase())
            {
                return Err(
                    "A sound with that name already exists; choose a different name".into(),
                );
            }
            next.insert(
                name,
                project
                    .document
                    .graph
                    .sound_definition(sound)
                    .cloned()
                    .expect("sound node should own a definition"),
            );
        }
        SoundLibraryCommand::Rename { name, new_name } => {
            let new_name = sound_library_name(new_name)?;
            if *name == new_name {
                return Ok(None);
            }
            if next.keys().any(|existing| {
                existing != name && existing.to_lowercase() == new_name.to_lowercase()
            }) {
                return Err("A sound with that name already exists".into());
            }
            let instrument = next
                .remove(name)
                .ok_or("That saved sound no longer exists")?;
            next.insert(new_name, instrument);
        }
        SoundLibraryCommand::Delete { name } => {
            next.remove(name)
                .ok_or("That saved sound no longer exists")?;
        }
    }
    validate_sound_library(&next, project.samples.manifest())?;
    if next == project.sound_library {
        return Ok(None);
    }
    let previous = std::mem::replace(&mut project.sound_library, next);
    project.mark_dirty();
    Ok(Some(previous))
}
