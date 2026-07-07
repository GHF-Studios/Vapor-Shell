//! Parsing for identity-bearing `Vapor.toml` files.
//!
//! A source manifest declares one local identity with `name`. Fully-qualified
//! identifiers are inferred from the nearest source root identity and are used
//! only by references to other Vapor artifacts.

use serde::Deserialize;
use std::{fmt, fs, path::Path};

/// Canonical filename for Vapor source markers.
pub const FILE_NAME: &str = "Vapor.toml";

/// Canonical content kinds shared by Vapor manifests and shell context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    /// Runtime engine source.
    Engine,
    /// Game source.
    Game,
    /// Root playable composition pack.
    Packagepack,
    /// Pack containing one engine and compatible engine mods.
    Enginepack,
    /// Pack containing one game and compatible game mods.
    Gamepack,
    /// Pack containing compatible mods.
    Modpack,
    /// Mod that targets an engine.
    EngineMod,
    /// Mod that targets a game.
    GameMod,
    /// Extension mod shared across supported targets.
    ExtensionMod,
}

impl fmt::Display for ContentKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Engine => "engine",
            Self::Game => "game",
            Self::Packagepack => "packagepack",
            Self::Enginepack => "enginepack",
            Self::Gamepack => "gamepack",
            Self::Modpack => "modpack",
            Self::EngineMod => "engine-mod",
            Self::GameMod => "game-mod",
            Self::ExtensionMod => "extension-mod",
        })
    }
}

/// The single identity declared by one marker file.
#[derive(Debug, Clone)]
pub enum VaporEntity {
    /// The source and Steam-depot root of the Vapor application itself.
    Root {
        /// Inferred identifier, `organization/name`.
        id: String,
        /// Local source-root name.
        name: String,
        /// Owning organization namespace.
        organization: String,
    },
    /// Registry authority root.
    Registry {
        /// Inferred identifier, `organization/name`.
        id: String,
        /// Local registry name.
        name: String,
        /// Owning organization namespace.
        organization: String,
    },
    /// A normal source workspace.
    Workspace {
        /// Inferred identifier, `organization/name`.
        id: String,
        /// Local workspace name.
        name: String,
        /// Owning organization namespace.
        organization: String,
    },
    /// One non-content Cargo package inside a source root.
    Project {
        /// Inferred identifier, `organization/workspace/project`.
        id: String,
        /// Local project name.
        name: String,
    },
    /// One content Cargo package inside a source root.
    Content {
        /// Canonical content category.
        kind: ContentKind,
        /// Inferred identifier, `organization/workspace/content`.
        id: String,
        /// Local content name.
        name: String,
    },
}

impl VaporEntity {
    /// Inferred fully-qualified identifier.
    pub fn id(&self) -> &str {
        match self {
            Self::Root { id, .. }
            | Self::Registry { id, .. }
            | Self::Workspace { id, .. }
            | Self::Project { id, .. }
            | Self::Content { id, .. } => id,
        }
    }
}

/// Read one marker while rejecting manifest symlinks that escape `source_root`.
///
/// # Errors
///
/// Fails when the marker cannot be resolved, escapes `source_root`, contains
/// invalid TOML, declares zero or multiple identity sections, or uses removed
/// declaration-side `id` fields.
pub fn read(path: &Path, source_root: &Path) -> Result<VaporEntity, String> {
    let canonical_path = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve '{}': {error}", path.display()))?;

    if !canonical_path.starts_with(source_root) {
        return Err(format!(
            "Vapor source boundary violation: '{}' is outside '{}'",
            canonical_path.display(),
            source_root.display()
        ));
    }
    if !canonical_path.is_file() {
        return Err(format!(
            "metadata path is not a file: {}",
            canonical_path.display()
        ));
    }

    let canonical_root_marker = fs::canonicalize(source_root.join(FILE_NAME)).ok();
    let is_root_marker = canonical_root_marker
        .as_ref()
        .is_some_and(|root_marker| root_marker == &canonical_path);
    let prefix = if is_root_marker {
        None
    } else {
        Some(read_source_prefix(source_root)?)
    };

    let source = fs::read_to_string(&canonical_path)
        .map_err(|error| format!("failed to read '{}': {error}", canonical_path.display()))?;
    let document: VaporToml = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", canonical_path.display()))?;

    document.into_entity(path, prefix.as_deref())
}

fn read_source_prefix(source_root: &Path) -> Result<String, String> {
    let path = source_root.join(FILE_NAME);
    let source = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read source root manifest '{}': {error}",
            path.display()
        )
    })?;
    let document: VaporToml = toml::from_str(&source).map_err(|error| {
        format!(
            "failed to parse source root manifest '{}': {error}",
            path.display()
        )
    })?;
    document.source_identity(&path)?.ok_or_else(|| {
        format!(
            "source root manifest '{}' must declare [root], [workspace], or [registry]",
            path.display()
        )
    })
}

#[derive(Debug, Deserialize)]
struct VaporToml {
    root: Option<GlobalMetadata>,
    registry: Option<GlobalMetadata>,
    workspace: Option<GlobalMetadata>,
    project: Option<LocalMetadata>,
    engine: Option<LocalMetadata>,
    game: Option<LocalMetadata>,
    packagepack: Option<LocalMetadata>,
    enginepack: Option<LocalMetadata>,
    gamepack: Option<LocalMetadata>,
    modpack: Option<LocalMetadata>,
    #[serde(rename = "engine-mod")]
    engine_mod: Option<LocalMetadata>,
    #[serde(rename = "game-mod")]
    game_mod: Option<LocalMetadata>,
    #[serde(rename = "extension-mod")]
    extension_mod: Option<LocalMetadata>,
}

impl VaporToml {
    fn source_identity(&self, path: &Path) -> Result<Option<String>, String> {
        let mut identities = Vec::new();
        if let Some(metadata) = &self.root {
            let (name, organization) = validate_global(metadata, "[root]", path)?;
            identities.push(format!("{organization}/{name}"));
        }
        if let Some(metadata) = &self.registry {
            let (name, organization) = validate_global(metadata, "[registry]", path)?;
            identities.push(format!("{organization}/{name}"));
        }
        if let Some(metadata) = &self.workspace {
            let (name, organization) = validate_global(metadata, "[workspace]", path)?;
            identities.push(format!("{organization}/{name}"));
        }

        match identities.len() {
            0 => Ok(None),
            1 => Ok(identities.pop()),
            _ => Err(format!(
                "'{}' declares multiple source-root identities; choose exactly one of [root], [workspace], or [registry]",
                path.display()
            )),
        }
    }

    fn into_entity(self, path: &Path, prefix: Option<&str>) -> Result<VaporEntity, String> {
        let mut entities = Vec::new();

        if let Some(metadata) = self.root {
            let (name, organization) = validate_global(&metadata, "[root]", path)?;
            entities.push(VaporEntity::Root {
                id: format!("{organization}/{name}"),
                name,
                organization,
            });
        }
        if let Some(metadata) = self.registry {
            let (name, organization) = validate_global(&metadata, "[registry]", path)?;
            entities.push(VaporEntity::Registry {
                id: format!("{organization}/{name}"),
                name,
                organization,
            });
        }
        if let Some(metadata) = self.workspace {
            let (name, organization) = validate_global(&metadata, "[workspace]", path)?;
            entities.push(VaporEntity::Workspace {
                id: format!("{organization}/{name}"),
                name,
                organization,
            });
        }

        collect_project(&mut entities, self.project, prefix, path)?;
        collect_content(
            &mut entities,
            self.engine,
            ContentKind::Engine,
            prefix,
            path,
        )?;
        collect_content(&mut entities, self.game, ContentKind::Game, prefix, path)?;
        collect_content(
            &mut entities,
            self.packagepack,
            ContentKind::Packagepack,
            prefix,
            path,
        )?;
        collect_content(
            &mut entities,
            self.enginepack,
            ContentKind::Enginepack,
            prefix,
            path,
        )?;
        collect_content(
            &mut entities,
            self.gamepack,
            ContentKind::Gamepack,
            prefix,
            path,
        )?;
        collect_content(
            &mut entities,
            self.modpack,
            ContentKind::Modpack,
            prefix,
            path,
        )?;
        collect_content(
            &mut entities,
            self.engine_mod,
            ContentKind::EngineMod,
            prefix,
            path,
        )?;
        collect_content(
            &mut entities,
            self.game_mod,
            ContentKind::GameMod,
            prefix,
            path,
        )?;
        collect_content(
            &mut entities,
            self.extension_mod,
            ContentKind::ExtensionMod,
            prefix,
            path,
        )?;

        match entities.len() {
            1 => Ok(entities.pop().expect("length was checked")),
            0 => Err(format!(
                "'{}' has no Vapor identity section; expected [root], [registry], [workspace], [project], or a content section",
                path.display()
            )),
            _ => Err(format!(
                "'{}' declares multiple Vapor identities; each {FILE_NAME} must describe exactly one source root, project, or content root",
                path.display()
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct GlobalMetadata {
    name: String,
    organization: String,
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LocalMetadata {
    name: String,
    id: Option<String>,
    kind: Option<String>,
}

fn collect_project(
    entities: &mut Vec<VaporEntity>,
    metadata: Option<LocalMetadata>,
    prefix: Option<&str>,
    path: &Path,
) -> Result<(), String> {
    if let Some(metadata) = metadata {
        let name = validate_local(&metadata, "[project]", path)?;
        let prefix = require_prefix(prefix, "[project]", path)?;
        entities.push(VaporEntity::Project {
            id: format!("{prefix}/{name}"),
            name,
        });
    }
    Ok(())
}

fn collect_content(
    entities: &mut Vec<VaporEntity>,
    metadata: Option<LocalMetadata>,
    kind: ContentKind,
    prefix: Option<&str>,
    path: &Path,
) -> Result<(), String> {
    if let Some(metadata) = metadata {
        let section = format!("[{kind}]");
        let name = validate_local(&metadata, &section, path)?;
        let prefix = require_prefix(prefix, &section, path)?;
        entities.push(VaporEntity::Content {
            kind,
            id: format!("{prefix}/{name}"),
            name,
        });
    }
    Ok(())
}

fn require_prefix<'a>(
    prefix: Option<&'a str>,
    section: &str,
    path: &Path,
) -> Result<&'a str, String> {
    prefix.ok_or_else(|| {
        format!(
            "{section} in '{}' cannot be the source root identity; declare [root] or [workspace] at the source root and move {section} into a Cargo package directory",
            path.display()
        )
    })
}

fn validate_global(
    metadata: &GlobalMetadata,
    section: &str,
    path: &Path,
) -> Result<(String, String), String> {
    reject_id(metadata.id.as_deref(), section, path)?;
    Ok((
        validate_name(&metadata.name, &format!("{section} name"), path)?,
        validate_name(
            &metadata.organization,
            &format!("{section} organization"),
            path,
        )?,
    ))
}

fn validate_local(metadata: &LocalMetadata, section: &str, path: &Path) -> Result<String, String> {
    reject_id(metadata.id.as_deref(), section, path)?;
    if metadata.kind.is_some() {
        return Err(format!(
            "{section} in '{}' uses removed field `kind`; project/content role is selected by the table name",
            path.display()
        ));
    }
    validate_name(&metadata.name, &format!("{section} name"), path)
}

fn reject_id(id: Option<&str>, section: &str, path: &Path) -> Result<(), String> {
    if id.is_some() {
        Err(format!(
            "{section} in '{}' uses removed field `id`; declarations use `name`, and full IDs are inferred",
            path.display()
        ))
    } else {
        Ok(())
    }
}

fn validate_name(value: &str, label: &str, path: &Path) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} in '{}' cannot be empty", path.display()));
    }
    if trimmed.starts_with('-')
        || trimmed.ends_with('-')
        || trimmed.chars().any(|character| {
            !character.is_ascii_lowercase() && !character.is_ascii_digit() && character != '-'
        })
    {
        return Err(format!(
            "{label} in '{}' must be lowercase kebab-case: {trimmed}",
            path.display()
        ));
    }
    Ok(trimmed.to_owned())
}
