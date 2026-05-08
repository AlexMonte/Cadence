use crate::adapter::backend;
use crate::application::editor::{EditorShellState, SAMPLE_LIBRARY_FOCUS_RULES};

pub(crate) fn filtered_sample_library_entries(
    snapshot: &EditorShellState,
) -> Vec<backend::SampleLibraryEntryDto> {
    let query = snapshot
        .sample_library_query_input
        .trim()
        .to_ascii_lowercase();
    snapshot
        .sample_library
        .entries
        .iter()
        .filter(|entry| sample_library_query_matches(query.as_str(), entry))
        .cloned()
        .collect()
}

fn sample_library_query_matches(query: &str, entry: &backend::SampleLibraryEntryDto) -> bool {
    if query.is_empty() {
        return true;
    }

    sample_library_display_name(entry)
        .to_ascii_lowercase()
        .contains(query)
        || entry.key.to_ascii_lowercase().contains(query)
        || entry
            .library
            .as_deref()
            .is_some_and(|library| library.to_ascii_lowercase().contains(query))
        || entry
            .tags
            .iter()
            .any(|tag| tag.to_ascii_lowercase().contains(query))
}

pub(crate) fn sample_library_sound_hint(entry: &backend::SampleLibraryEntryDto) -> String {
    format!(
        "sound.value = {}",
        sample_sound_value_expression(entry.key.as_str())
    )
}

pub(crate) fn sample_library_display_name(entry: &backend::SampleLibraryEntryDto) -> String {
    let base_name = entry.key.rsplit('/').next().unwrap_or(entry.key.as_str());
    let base_tokens = sample_library_name_tokens(base_name);
    let focus = sample_library_focus_rule(entry, &base_tokens);
    let focus_label = focus.map(|(label, _)| label);
    let focus_aliases = focus.map(|(_, aliases)| aliases);
    let descriptor = sample_library_descriptor(entry, focus_aliases, &base_tokens);

    match (focus_label, descriptor) {
        (Some(label), Some(descriptor)) => format!("{label}: {descriptor}"),
        (Some(label), None) => label.to_string(),
        (None, Some(descriptor)) => descriptor,
        (None, None) => base_name.to_string(),
    }
}

fn sample_library_focus_rule(
    entry: &backend::SampleLibraryEntryDto,
    base_tokens: &[String],
) -> Option<(&'static str, &'static [&'static str])> {
    let search_tokens = base_tokens
        .iter()
        .map(String::as_str)
        .chain(entry.tags.iter().map(String::as_str))
        .collect::<Vec<_>>();

    SAMPLE_LIBRARY_FOCUS_RULES
        .iter()
        .find_map(|(label, aliases)| {
            aliases
                .iter()
                .any(|alias| search_tokens.iter().any(|token| token == alias))
                .then_some((*label, *aliases))
        })
}

fn sample_library_descriptor(
    entry: &backend::SampleLibraryEntryDto,
    focus_aliases: Option<&[&str]>,
    base_tokens: &[String],
) -> Option<String> {
    let descriptor = sample_library_humanize_tokens(sample_library_without_focus_tokens(
        base_tokens,
        focus_aliases,
    ));
    if descriptor.is_some() {
        return descriptor;
    }

    let context = entry
        .key
        .rsplit('/')
        .nth(1)
        .map(sample_library_name_tokens)
        .unwrap_or_default();
    sample_library_humanize_tokens(sample_library_without_focus_tokens(&context, focus_aliases))
}

fn sample_library_without_focus_tokens<'a>(
    tokens: &'a [String],
    focus_aliases: Option<&[&str]>,
) -> Vec<&'a str> {
    let trim_hat_descriptors = focus_aliases
        .map(|aliases| aliases.iter().any(|alias| *alias == "hat"))
        .unwrap_or(false);
    let mut filtered = tokens
        .iter()
        .map(String::as_str)
        .filter(|token| {
            focus_aliases
                .map(|aliases| !aliases.iter().any(|alias| alias == token))
                .unwrap_or(true)
                && !(trim_hat_descriptors && matches!(*token, "hi" | "open"))
        })
        .collect::<Vec<_>>();
    filtered.dedup();
    filtered
}

fn sample_library_humanize_tokens(tokens: Vec<&str>) -> Option<String> {
    (!tokens.is_empty()).then(|| {
        tokens
            .into_iter()
            .map(sample_library_humanize_token)
            .collect::<Vec<_>>()
            .join(" ")
    })
}

fn sample_library_humanize_token(token: &str) -> String {
    if token == "808" {
        return "808".to_string();
    }

    let mut characters = token.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    let rest = characters.collect::<String>();
    format!(
        "{}{}",
        first.to_ascii_uppercase(),
        rest.to_ascii_lowercase()
    )
}

fn sample_library_name_tokens(value: &str) -> Vec<String> {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(|token| token.to_string())
        .collect()
}

pub(crate) fn sample_library_bank_hint(entry: &backend::SampleLibraryEntryDto) -> Option<String> {
    entry
        .library
        .as_ref()
        .map(|library| format!("bank.value = {:?}", library))
}

pub(crate) fn sample_library_tags_hint(entry: &backend::SampleLibraryEntryDto) -> Option<String> {
    (!entry.tags.is_empty()).then(|| {
        entry
            .tags
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    })
}

pub(crate) fn sample_load_sound_hint(sample: &backend::CadenceSampleLoadDto) -> String {
    format!(
        "sound.value = {}",
        sample_sound_value_expression(sample.id.as_str())
    )
}

pub(crate) fn sample_load_alias_hint(sample: &backend::CadenceSampleLoadDto) -> Option<String> {
    let aliases = sample.aliases.keys().cloned().collect::<Vec<_>>();
    (!aliases.is_empty()).then(|| {
        aliases
            .iter()
            .map(|alias| sample_sound_value_expression(alias.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    })
}

fn sample_sound_value_expression(value: &str) -> String {
    if sample_sound_value_requires_quotes(value) {
        format!("\"{}\"", escape_sample_sound_value(value))
    } else {
        value.to_string()
    }
}

fn sample_sound_value_requires_quotes(value: &str) -> bool {
    value.is_empty()
        || value.chars().any(|character| {
            character.is_whitespace()
                || matches!(
                    character,
                    '"' | '[' | ']' | '{' | '}' | '|' | ',' | '*' | '/' | '^' | '.' | '_'
                )
        })
}

fn escape_sample_sound_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' | '\\' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}
