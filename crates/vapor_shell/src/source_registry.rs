//! App-local index of external Vapor source roots.
//!
//! The registry stores source roots by a short local name inferred from their
//! Vapor identity. It lives in the Steam installation/app root because it is
//! tool state, not authored source.

use crate::discovery::{InstallationPaths, SourceWorkspace};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

const ACTIVE_SOURCE: &str = "source-workspace";
const SOURCE_REGISTRY: &str = "sources.toml";

/// One indexed external source root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEntry {
    id: String,
    path: PathBuf,
}

impl SourceEntry {
    /// Fully-qualified source identity.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Canonical source root path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Sorted app-local source index.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceRegistry {
    sources: BTreeMap<String, SourceEntry>,
}

impl SourceRegistry {
    /// Indexed sources keyed by local/bare source name.
    pub fn sources(&self) -> &BTreeMap<String, SourceEntry> {
        &self.sources
    }
}

/// Load the app-local source registry.
///
/// # Errors
///
/// Returns read or TOML parse errors.
pub fn load(installation: &InstallationPaths) -> Result<SourceRegistry, String> {
    let path = registry_path(installation);
    if !path.is_file() {
        return Ok(SourceRegistry::default());
    }
    let source = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read source registry '{}': {error}",
            path.display()
        )
    })?;
    toml::from_str(&source).map_err(|error| {
        format!(
            "failed to parse source registry '{}': {error}",
            path.display()
        )
    })
}

/// Persist one validated source root in the app-local registry.
///
/// # Errors
///
/// Returns filesystem or TOML encoding errors.
pub fn add(
    installation: &InstallationPaths,
    source: &SourceWorkspace,
) -> Result<(String, SourceEntry), String> {
    let mut registry = load(installation)?;
    let name = local_name(source.identity_id()).to_owned();
    let entry = SourceEntry {
        id: source.identity_id().to_owned(),
        path: source.root().to_path_buf(),
    };
    registry.sources.insert(name.clone(), entry.clone());
    save(installation, &registry)?;
    Ok((name, entry))
}

/// Remove one indexed source by local name or full ID.
///
/// # Errors
///
/// Returns filesystem or TOML errors.
pub fn remove(installation: &InstallationPaths, selector: &str) -> Result<Option<String>, String> {
    let mut registry = load(installation)?;
    let key = resolve_key(&registry, selector);
    let removed = key.and_then(|key| registry.sources.remove(&key).map(|_| key));
    if removed.is_some() {
        save(installation, &registry)?;
    }
    Ok(removed)
}

/// Resolve an `open` target as either a path or an indexed source name/ID.
///
/// # Errors
///
/// Returns a clear diagnostic when a non-path target is not indexed.
pub fn resolve_target(installation: &InstallationPaths, target: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(target);
    if candidate.exists() || target.contains('/') || target.contains('\\') || target == "." {
        return Ok(candidate);
    }
    let registry = load(installation)?;
    resolve_key(&registry, target)
        .and_then(|key| registry.sources.get(&key))
        .map(|entry| entry.path.clone())
        .ok_or_else(|| {
            format!(
                "unknown Vapor source '{target}'\nhelp: index it with `vapor source add PATH`, or open by path"
            )
        })
}

/// Read the active source selection from env or app-local state.
///
/// # Errors
///
/// Returns read errors for persisted state.
pub fn active_source(installation: &InstallationPaths) -> Result<Option<PathBuf>, String> {
    if let Some(path) = env::var_os("VAPOR_WORKSPACE").filter(|value| !value.is_empty()) {
        return Ok(Some(PathBuf::from(path)));
    }
    let path = active_path(installation);
    if !path.is_file() {
        return Ok(None);
    }
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read active source '{}': {error}", path.display()))?;
    let source = source.trim();
    if source.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(source)))
    }
}

/// Persist the active source root used by future app-first launches.
///
/// # Errors
///
/// Returns filesystem errors.
pub fn set_active(
    installation: &InstallationPaths,
    source: &SourceWorkspace,
) -> Result<(), String> {
    let path = active_path(installation);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create source state directory '{}': {error}",
                parent.display()
            )
        })?;
    }
    fs::write(&path, source.root().display().to_string()).map_err(|error| {
        format!(
            "failed to write active source '{}': {error}",
            path.display()
        )
    })
}

/// Clear the persisted active source selection.
///
/// # Errors
///
/// Returns filesystem errors.
pub fn clear_active(installation: &InstallationPaths) -> Result<(), String> {
    let path = active_path(installation);
    if path.exists() {
        fs::remove_file(&path).map_err(|error| {
            format!(
                "failed to remove active source '{}': {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn save(installation: &InstallationPaths, registry: &SourceRegistry) -> Result<(), String> {
    let path = registry_path(installation);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create source registry directory '{}': {error}",
                parent.display()
            )
        })?;
    }
    let source = toml::to_string_pretty(registry)
        .map_err(|error| format!("failed to encode source registry: {error}"))?;
    fs::write(&path, source).map_err(|error| {
        format!(
            "failed to write source registry '{}': {error}",
            path.display()
        )
    })
}

fn registry_path(installation: &InstallationPaths) -> PathBuf {
    installation.state_dir().join(SOURCE_REGISTRY)
}

fn active_path(installation: &InstallationPaths) -> PathBuf {
    installation.state_dir().join(ACTIVE_SOURCE)
}

fn resolve_key(registry: &SourceRegistry, selector: &str) -> Option<String> {
    if registry.sources.contains_key(selector) {
        return Some(selector.to_owned());
    }
    registry
        .sources
        .iter()
        .find_map(|(name, entry)| (entry.id == selector).then(|| name.clone()))
}

fn local_name(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id)
}
