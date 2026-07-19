//! Parsing for identity-bearing Vapor manifest files.
//!
//! Vapor manifest filenames are role-specific. The filename selects the role,
//! and the TOML table inside the file must match that role. This keeps source
//! roots, installed app roots, registries, workspaces, and content artifacts
//! visibly distinct at the filesystem level.

use serde::Deserialize;
use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

/// Canonical filename for the Vapor application source/depot authority.
pub const APP_SOURCE_FILE_NAME: &str = "App-Source.vapor.toml";
/// Canonical filename for the installed Vapor application runtime manifest.
pub const APP_FILE_NAME: &str = "App.vapor.toml";
/// Canonical filename for a normal source workspace.
pub const WORKSPACE_FILE_NAME: &str = "Workspace.vapor.toml";
/// Canonical filename for a Vapor Registry checkout.
pub const REGISTRY_FILE_NAME: &str = "Registry.vapor.toml";
/// Canonical filename for an engine content root.
pub const ENGINE_FILE_NAME: &str = "Engine.vapor.toml";
/// Canonical filename for a game content root.
pub const GAME_FILE_NAME: &str = "Game.vapor.toml";
/// Canonical filename for a packagepack content root.
pub const PACKAGEPACK_FILE_NAME: &str = "Packagepack.vapor.toml";
/// Canonical filename for an enginepack content root.
pub const ENGINEPACK_FILE_NAME: &str = "Enginepack.vapor.toml";
/// Canonical filename for a gamepack content root.
pub const GAMEPACK_FILE_NAME: &str = "Gamepack.vapor.toml";
/// Canonical filename for a modpack content root.
pub const MODPACK_FILE_NAME: &str = "Modpack.vapor.toml";
/// Canonical filename for an engine-mod content root.
pub const ENGINE_MOD_FILE_NAME: &str = "Engine-Mod.vapor.toml";
/// Canonical filename for a game-mod content root.
pub const GAME_MOD_FILE_NAME: &str = "Game-Mod.vapor.toml";
/// Canonical filename for an extension-mod content root.
pub const EXTENSION_MOD_FILE_NAME: &str = "Extension-Mod.vapor.toml";

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

impl ContentKind {
    /// Role-specific manifest filename for this content kind.
    pub fn manifest_file_name(self) -> &'static str {
        match self {
            Self::Engine => ENGINE_FILE_NAME,
            Self::Game => GAME_FILE_NAME,
            Self::Packagepack => PACKAGEPACK_FILE_NAME,
            Self::Enginepack => ENGINEPACK_FILE_NAME,
            Self::Gamepack => GAMEPACK_FILE_NAME,
            Self::Modpack => MODPACK_FILE_NAME,
            Self::EngineMod => ENGINE_MOD_FILE_NAME,
            Self::GameMod => GAME_MOD_FILE_NAME,
            Self::ExtensionMod => EXTENSION_MOD_FILE_NAME,
        }
    }
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
            | Self::Content { id, .. } => id,
        }
    }
}

/// Read one role-specific Vapor manifest while rejecting symlinks that escape
/// `source_root`.
///
/// # Errors
///
/// Fails when the marker cannot be resolved, escapes `source_root`, uses an
/// unknown filename, contains invalid TOML, declares the wrong section for its
/// filename, declares multiple identity sections, or uses removed
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

    let role = ManifestRole::from_path(&canonical_path)?;
    let is_root_marker = root_marker_candidates(source_root)
        .into_iter()
        .filter_map(|marker| fs::canonicalize(marker).ok())
        .any(|root_marker| root_marker == canonical_path);
    let prefix = if is_root_marker {
        None
    } else {
        Some(read_source_prefix(source_root)?)
    };

    let source = fs::read_to_string(&canonical_path)
        .map_err(|error| format!("failed to read '{}': {error}", canonical_path.display()))?;
    let document: VaporToml = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", canonical_path.display()))?;

    document.into_entity(role, path, prefix.as_deref())
}

fn read_source_prefix(source_root: &Path) -> Result<String, String> {
    let path = source_root_marker(source_root)?;
    let source = fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read source root manifest '{}': {error}",
            path.display()
        )
    })?;
    let role = ManifestRole::from_path(&path)?;
    let document: VaporToml = toml::from_str(&source).map_err(|error| {
        format!(
            "failed to parse source root manifest '{}': {error}",
            path.display()
        )
    })?;
    document.source_identity(role, &path)
}

/// Resolve the manifest that identifies a source root.
///
/// Application source roots use [`APP_SOURCE_FILE_NAME`], normal workspaces use
/// [`WORKSPACE_FILE_NAME`], and registry roots use [`REGISTRY_FILE_NAME`].
/// Installed app roots use [`APP_FILE_NAME`] and are intentionally not source
/// roots.
///
/// # Errors
///
/// Fails when no recognized source-root marker exists at `root`.
pub fn source_root_marker(root: &Path) -> Result<PathBuf, String> {
    source_root_marker_candidates(root)
        .into_iter()
        .find(|marker| marker.is_file())
        .ok_or_else(|| {
            format!(
                "no Vapor source-root manifest found in '{}'; expected {}, {}, or {}",
                root.display(),
                APP_SOURCE_FILE_NAME,
                WORKSPACE_FILE_NAME,
                REGISTRY_FILE_NAME
            )
        })
}

/// Recognized source-root manifest paths in precedence order.
pub fn source_root_marker_candidates(root: &Path) -> Vec<PathBuf> {
    [
        APP_SOURCE_FILE_NAME,
        WORKSPACE_FILE_NAME,
        REGISTRY_FILE_NAME,
    ]
    .into_iter()
    .map(|name| root.join(name))
    .collect()
}

/// Recognized root-level manifest paths in precedence order.
pub fn root_marker_candidates(root: &Path) -> Vec<PathBuf> {
    [
        APP_SOURCE_FILE_NAME,
        APP_FILE_NAME,
        WORKSPACE_FILE_NAME,
        REGISTRY_FILE_NAME,
    ]
    .into_iter()
    .map(|name| root.join(name))
    .collect()
}

/// Role-specific content manifest paths in deterministic order.
pub fn content_marker_candidates(root: &Path) -> Vec<PathBuf> {
    [
        ENGINE_FILE_NAME,
        GAME_FILE_NAME,
        PACKAGEPACK_FILE_NAME,
        ENGINEPACK_FILE_NAME,
        GAMEPACK_FILE_NAME,
        MODPACK_FILE_NAME,
        ENGINE_MOD_FILE_NAME,
        GAME_MOD_FILE_NAME,
        EXTENSION_MOD_FILE_NAME,
    ]
    .into_iter()
    .map(|name| root.join(name))
    .collect()
}

/// Locate a role-specific content manifest in one directory.
pub fn content_marker_at(root: &Path) -> Option<PathBuf> {
    content_marker_candidates(root)
        .into_iter()
        .find(|marker| marker.is_file())
}

#[derive(Debug, Clone, Copy)]
enum ManifestRole {
    AppSource,
    App,
    Registry,
    Workspace,
    Content(ContentKind),
}

impl ManifestRole {
    fn from_path(path: &Path) -> Result<Self, String> {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            return Err(format!("manifest path has no filename: {}", path.display()));
        };
        match file_name {
            APP_SOURCE_FILE_NAME => Ok(Self::AppSource),
            APP_FILE_NAME => Ok(Self::App),
            REGISTRY_FILE_NAME => Ok(Self::Registry),
            WORKSPACE_FILE_NAME => Ok(Self::Workspace),
            ENGINE_FILE_NAME => Ok(Self::Content(ContentKind::Engine)),
            GAME_FILE_NAME => Ok(Self::Content(ContentKind::Game)),
            PACKAGEPACK_FILE_NAME => Ok(Self::Content(ContentKind::Packagepack)),
            ENGINEPACK_FILE_NAME => Ok(Self::Content(ContentKind::Enginepack)),
            GAMEPACK_FILE_NAME => Ok(Self::Content(ContentKind::Gamepack)),
            MODPACK_FILE_NAME => Ok(Self::Content(ContentKind::Modpack)),
            ENGINE_MOD_FILE_NAME => Ok(Self::Content(ContentKind::EngineMod)),
            GAME_MOD_FILE_NAME => Ok(Self::Content(ContentKind::GameMod)),
            EXTENSION_MOD_FILE_NAME => Ok(Self::Content(ContentKind::ExtensionMod)),
            other => Err(format!(
                "unknown Vapor manifest filename '{}' at '{}'; use role-specific *.vapor.toml filenames",
                other,
                path.display()
            )),
        }
    }

    fn expected_section(self) -> &'static str {
        match self {
            Self::AppSource | Self::App => "root",
            Self::Registry => "registry",
            Self::Workspace => "workspace",
            Self::Content(ContentKind::Engine) => "engine",
            Self::Content(ContentKind::Game) => "game",
            Self::Content(ContentKind::Packagepack) => "packagepack",
            Self::Content(ContentKind::Enginepack) => "enginepack",
            Self::Content(ContentKind::Gamepack) => "gamepack",
            Self::Content(ContentKind::Modpack) => "modpack",
            Self::Content(ContentKind::EngineMod) => "engine-mod",
            Self::Content(ContentKind::GameMod) => "game-mod",
            Self::Content(ContentKind::ExtensionMod) => "extension-mod",
        }
    }
}

#[derive(Debug, Deserialize)]
struct VaporToml {
    root: Option<GlobalMetadata>,
    registry: Option<GlobalMetadata>,
    workspace: Option<GlobalMetadata>,
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
    fn source_identity(&self, role: ManifestRole, path: &Path) -> Result<String, String> {
        self.reject_extra_sections(role, path)?;
        match role {
            ManifestRole::AppSource => {
                let metadata = self.required_global("root", path)?;
                let (name, organization) = validate_global(metadata, "[root]", path)?;
                Ok(format!("{organization}/{name}"))
            }
            ManifestRole::Registry => {
                let metadata = self.required_global("registry", path)?;
                let (name, organization) = validate_global(metadata, "[registry]", path)?;
                Ok(format!("{organization}/{name}"))
            }
            ManifestRole::Workspace => {
                let metadata = self.required_global("workspace", path)?;
                let (name, organization) = validate_global(metadata, "[workspace]", path)?;
                Ok(format!("{organization}/{name}"))
            }
            ManifestRole::App | ManifestRole::Content(_) => Err(format!(
                "'{}' cannot identify a source root; expected {}, {}, or {}",
                path.display(),
                APP_SOURCE_FILE_NAME,
                WORKSPACE_FILE_NAME,
                REGISTRY_FILE_NAME
            )),
        }
    }

    fn into_entity(
        self,
        role: ManifestRole,
        path: &Path,
        prefix: Option<&str>,
    ) -> Result<VaporEntity, String> {
        self.reject_extra_sections(role, path)?;
        match role {
            ManifestRole::AppSource | ManifestRole::App => {
                let metadata = self.required_global("root", path)?;
                let (name, organization) = validate_global(metadata, "[root]", path)?;
                Ok(VaporEntity::Root {
                    id: format!("{organization}/{name}"),
                    name,
                    organization,
                })
            }
            ManifestRole::Registry => {
                let metadata = self.required_global("registry", path)?;
                let (name, organization) = validate_global(metadata, "[registry]", path)?;
                Ok(VaporEntity::Registry {
                    id: format!("{organization}/{name}"),
                    name,
                    organization,
                })
            }
            ManifestRole::Workspace => {
                let metadata = self.required_global("workspace", path)?;
                let (name, organization) = validate_global(metadata, "[workspace]", path)?;
                Ok(VaporEntity::Workspace {
                    id: format!("{organization}/{name}"),
                    name,
                    organization,
                })
            }
            ManifestRole::Content(kind) => {
                let section = format!("[{kind}]");
                let metadata = self
                    .content_metadata(kind)
                    .ok_or_else(|| missing_section(role, path))?;
                let name = validate_local(metadata, &section, path)?;
                let prefix = require_prefix(prefix, &section, path)?;
                Ok(VaporEntity::Content {
                    kind,
                    id: format!("{prefix}/{name}"),
                    name,
                })
            }
        }
    }

    fn required_global(&self, section: &str, path: &Path) -> Result<&GlobalMetadata, String> {
        match section {
            "root" => self.root.as_ref(),
            "registry" => self.registry.as_ref(),
            "workspace" => self.workspace.as_ref(),
            _ => None,
        }
        .ok_or_else(|| missing_section_for_name(section, path))
    }

    fn content_metadata(&self, kind: ContentKind) -> Option<&LocalMetadata> {
        match kind {
            ContentKind::Engine => self.engine.as_ref(),
            ContentKind::Game => self.game.as_ref(),
            ContentKind::Packagepack => self.packagepack.as_ref(),
            ContentKind::Enginepack => self.enginepack.as_ref(),
            ContentKind::Gamepack => self.gamepack.as_ref(),
            ContentKind::Modpack => self.modpack.as_ref(),
            ContentKind::EngineMod => self.engine_mod.as_ref(),
            ContentKind::GameMod => self.game_mod.as_ref(),
            ContentKind::ExtensionMod => self.extension_mod.as_ref(),
        }
    }

    fn reject_extra_sections(&self, role: ManifestRole, path: &Path) -> Result<(), String> {
        let expected = role.expected_section();
        let present = self.present_identity_sections();
        if present.is_empty() {
            return Err(missing_section(role, path));
        }
        if present.len() > 1 {
            return Err(format!(
                "'{}' declares multiple Vapor identity sections; {} must contain exactly [{}]",
                path.display(),
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("manifest"),
                expected
            ));
        }
        if present[0] != expected {
            return Err(format!(
                "'{}' declares [{}], but {} requires [{}]",
                path.display(),
                present[0],
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("manifest"),
                expected
            ));
        }
        Ok(())
    }

    fn present_identity_sections(&self) -> Vec<&'static str> {
        let mut sections = Vec::new();
        if self.root.is_some() {
            sections.push("root");
        }
        if self.registry.is_some() {
            sections.push("registry");
        }
        if self.workspace.is_some() {
            sections.push("workspace");
        }
        if self.engine.is_some() {
            sections.push("engine");
        }
        if self.game.is_some() {
            sections.push("game");
        }
        if self.packagepack.is_some() {
            sections.push("packagepack");
        }
        if self.enginepack.is_some() {
            sections.push("enginepack");
        }
        if self.gamepack.is_some() {
            sections.push("gamepack");
        }
        if self.modpack.is_some() {
            sections.push("modpack");
        }
        if self.engine_mod.is_some() {
            sections.push("engine-mod");
        }
        if self.game_mod.is_some() {
            sections.push("game-mod");
        }
        if self.extension_mod.is_some() {
            sections.push("extension-mod");
        }
        sections
    }
}

fn missing_section(role: ManifestRole, path: &Path) -> String {
    missing_section_for_name(role.expected_section(), path)
}

fn missing_section_for_name(section: &str, path: &Path) -> String {
    format!(
        "'{}' must declare [{}] because of its role-specific filename",
        path.display(),
        section
    )
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

fn require_prefix<'a>(
    prefix: Option<&'a str>,
    section: &str,
    path: &Path,
) -> Result<&'a str, String> {
    prefix.ok_or_else(|| {
        format!(
            "{section} in '{}' cannot be the source root identity; declare [root] in {}, or [workspace] in {}, and move {section} into a Cargo package directory",
            path.display(),
            APP_SOURCE_FILE_NAME,
            WORKSPACE_FILE_NAME
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
            "{section} in '{}' uses removed field `kind`; content role is selected by the manifest filename",
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
        return Err(format!("{label} in '{}' must not be empty", path.display()));
    }
    let valid = trimmed.chars().all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || matches!(character, '-' | '_')
    });
    if !valid {
        return Err(format!(
            "{label} in '{}' must use lowercase ASCII letters, digits, '-' or '_'",
            path.display()
        ));
    }
    Ok(trimmed.to_owned())
}
