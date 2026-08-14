use crate::{
    SchematicDefinition, SchematicLibrarySymbol, SchematicPlacedSymbol, SourceBundleError,
    SourceBundleErrorKind,
};
use std::collections::HashMap;

impl SchematicDefinition {
    pub fn library_symbol_for_placement(
        &self,
        symbol: &SchematicPlacedSymbol,
    ) -> Option<&SchematicLibrarySymbol> {
        self.library_symbol_index_for_key(&symbol.lib_name)
            .or_else(|| self.library_symbol_index_for_key(&symbol.lib_id))
            .map(|index| &self.library_symbols[index])
    }

    pub fn library_pin_symbol_for_placement(
        &self,
        symbol: &SchematicPlacedSymbol,
    ) -> Option<&SchematicLibrarySymbol> {
        let index = self
            .library_symbol_index_for_key(&symbol.lib_name)
            .or_else(|| self.library_symbol_index_for_key(&symbol.lib_id))?;
        self.library_pin_owner_by_symbol[index]
            .map(|owner_index| &self.library_symbols[owner_index])
    }

    fn library_symbol_index_for_key(&self, key: &str) -> Option<usize> {
        library_symbol_index_for_key(&self.library_symbol_by_key, key)
    }
}

/// Resolve the effective pin-bearing symbol for every library symbol once.
///
/// The inheritance graph is functional (each symbol has at most one `extends`
/// edge), so a three-state walk resolves every edge at most once. Missing
/// parents and cycles deliberately resolve to no pin owner.
pub(crate) fn resolve_library_pin_owners(
    symbols: &[SchematicLibrarySymbol],
    index: &HashMap<String, usize>,
) -> Vec<Option<usize>> {
    let mut owners = vec![None; symbols.len()];
    let mut states = vec![0_u8; symbols.len()];
    for start in 0..symbols.len() {
        if states[start] == 2 {
            continue;
        }
        let mut path = Vec::new();
        let mut current = start;
        let owner = loop {
            if states[current] == 2 {
                break owners[current];
            }
            if states[current] == 1 {
                break None;
            }
            states[current] = 1;
            path.push(current);
            let symbol = &symbols[current];
            if !symbol.subsymbols.is_empty() {
                break Some(current);
            }
            let Some(parent) = symbol
                .extends
                .as_deref()
                .and_then(|key| library_symbol_index_for_key(index, key))
            else {
                break None;
            };
            current = parent;
        };
        for symbol_index in path.into_iter().rev() {
            owners[symbol_index] = owner;
            states[symbol_index] = 2;
        }
    }
    owners
}

fn library_symbol_index_for_key(index: &HashMap<String, usize>, key: &str) -> Option<usize> {
    if key.is_empty() {
        return None;
    }
    let direct = index.get(key).copied();
    let basename = key
        .rsplit_once(':')
        .and_then(|(_, basename)| index.get(basename).copied());
    direct.into_iter().chain(basename).min()
}

pub(crate) fn index_bundle_legacy_instances(
    definitions: &[SchematicDefinition],
) -> HashMap<String, (usize, usize)> {
    let mut index = HashMap::new();
    for (definition_index, definition) in definitions.iter().enumerate() {
        for (instance_index, instance) in definition.legacy_symbol_instances.iter().enumerate() {
            let path = instance.path.trim_end_matches('/');
            if !path.is_empty() {
                index
                    .entry(path.to_owned())
                    .or_insert((definition_index, instance_index));
            }
        }
    }
    index
}

pub(crate) fn index_library_symbols(
    symbols: &[SchematicLibrarySymbol],
    source_path: &str,
    max_key_bytes: usize,
) -> Result<HashMap<String, usize>, SourceBundleError> {
    let mut index = HashMap::new();
    let mut retained_bytes = 0_usize;
    for (symbol_index, symbol) in symbols.iter().enumerate() {
        let mut keys = std::iter::once(symbol.name.as_str()).chain(
            symbol
                .name
                .char_indices()
                .filter(|(_, character)| *character == ':')
                .map(|(offset, _)| &symbol.name[offset + 1..]),
        );
        keys.try_for_each(|key| {
            retained_bytes = retained_bytes
                .checked_add(key.len())
                .ok_or_else(|| limit_error(source_path, "library lookup key bytes overflow"))?;
            if retained_bytes > max_key_bytes {
                return Err(limit_error(
                    source_path,
                    "library lookup key bytes exceed their limit",
                ));
            }
            index.entry(key.to_owned()).or_insert(symbol_index);
            Ok(())
        })?;
    }
    Ok(index)
}

pub(crate) fn portable_file_stem(path: &str) -> String {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .strip_suffix(".kicad_pro")
        .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path))
        .to_owned()
}

fn limit_error(source_path: &str, message: &str) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::ResourceLimit,
        Some(source_path),
        message,
    )
}
