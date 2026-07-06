//! Parsing for identity-bearing `Vapor.toml` files.
//!
//! A marker describes exactly one workspace, project, or content root. Tables such as
//! `[toolchain]` and `[[content]]` may coexist with that identity and are ignored
//! here because they belong to other Vapor subsystems.

use serde::Deserialize;
use std::{fmt, fs, path::Path};

/// Canonical filename for Vapor workspace and content markers.
pub const FILE_NAME: &str = "Vapor.toml";

/// Exhaustive project roles understood by Vapor tooling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectKind {
    /// Foundational Vapor runtime and shared contracts.
    Core,
    /// Authoring SDK workspace.
    Sdk,
    /// Player-facing launcher workspace.
    Launcher,
    /// User-authored engines, games, mods, and packs.
    CustomContent,
    /// Vapor shell installation or development workspace.
    Shell,
}

impl fmt::Display for ProjectKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Core => "core",
            Self::Sdk => "sdk",
            Self::Launcher => "launcher",
            Self::CustomContent => "custom-content",
            Self::Shell => "shell",
        })
    }
}

/// Canonical content kinds shared by Vapor manifests and shell context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    /// Runtime engine source.
    Engine,
    /// Game source.
    Game,
    /// Root playable composition pack.
    Packagepack,
    /// Pack containing engines, engine mods, and nested engine packs.
    Enginepack,
    /// Pack containing games, game mods, and nested game packs.
    Gamepack,
    /// Pack containing compatible mods and nested mod packs.
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
            Self::EngineMod => "engine_mod",
            Self::GameMod => "game_mod",
            Self::ExtensionMod => "extension_mod",
        })
    }
}

/// The single identity declared by one marker file.
#[derive(Debug)]
pub enum VaporEntity {
    /// A source workspace or replaceable installation root.
    Workspace {
        /// Stable workspace identifier.
        id: String,
    },
    /// A component repository contained by a workspace.
    Project {
        /// Validated project role.
        kind: ProjectKind,
        /// Stable project identifier.
        id: String,
    },
    /// One authored content root.
    Content {
        /// Canonical content category.
        kind: ContentKind,
        /// Stable content identifier.
        id: String,
    },
}

/// Read one marker while rejecting manifest symlinks that escape `workspace_root`.
///
/// # Errors
///
/// Fails when the marker cannot be resolved, escapes `workspace_root`, contains
/// invalid TOML, or declares zero or multiple identity sections.
pub fn read(path: &Path, workspace_root: &Path) -> Result<VaporEntity, String> {
    let canonical_path = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve '{}': {error}", path.display()))?;

    if !canonical_path.starts_with(workspace_root) {
        return Err(format!(
            "Vapor workspace boundary violation: '{}' is outside '{}'",
            canonical_path.display(),
            workspace_root.display()
        ));
    }
    if !canonical_path.is_file() {
        return Err(format!(
            "metadata path is not a file: {}",
            canonical_path.display()
        ));
    }

    let source = fs::read_to_string(&canonical_path)
        .map_err(|error| format!("failed to read '{}': {error}", canonical_path.display()))?;
    let document: VaporToml = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", canonical_path.display()))?;

    document.into_entity(path)
}

#[derive(Debug, Deserialize)]
struct VaporToml {
    workspace: Option<WorkspaceMetadata>,
    project: Option<ProjectMetadata>,
    engine: Option<ContentMetadata>,
    game: Option<ContentMetadata>,
    packagepack: Option<ContentMetadata>,
    enginepack: Option<ContentMetadata>,
    gamepack: Option<ContentMetadata>,
    modpack: Option<ContentMetadata>,
    engine_mod: Option<ContentMetadata>,
    game_mod: Option<ContentMetadata>,
    extension_mod: Option<ContentMetadata>,
}

impl VaporToml {
    fn into_entity(self, path: &Path) -> Result<VaporEntity, String> {
        let mut entities = Vec::new();

        if let Some(workspace) = self.workspace {
            entities.push(VaporEntity::Workspace {
                id: validate(workspace.id, "workspace id", path)?,
            });
        }
        if let Some(project) = self.project {
            entities.push(VaporEntity::Project {
                kind: project.kind,
                id: validate(project.id, "project id", path)?,
            });
        }

        collect_content(&mut entities, self.engine, ContentKind::Engine, path)?;
        collect_content(&mut entities, self.game, ContentKind::Game, path)?;
        collect_content(
            &mut entities,
            self.packagepack,
            ContentKind::Packagepack,
            path,
        )?;
        collect_content(
            &mut entities,
            self.enginepack,
            ContentKind::Enginepack,
            path,
        )?;
        collect_content(&mut entities, self.gamepack, ContentKind::Gamepack, path)?;
        collect_content(&mut entities, self.modpack, ContentKind::Modpack, path)?;
        collect_content(&mut entities, self.engine_mod, ContentKind::EngineMod, path)?;
        collect_content(&mut entities, self.game_mod, ContentKind::GameMod, path)?;
        collect_content(
            &mut entities,
            self.extension_mod,
            ContentKind::ExtensionMod,
            path,
        )?;

        match entities.len() {
            1 => Ok(entities.pop().expect("length was checked")),
            0 => Err(format!(
                "'{}' has no Vapor identity section; expected [workspace], [project], or a content section",
                path.display()
            )),
            _ => Err(format!(
                "'{}' declares multiple Vapor identities; each {FILE_NAME} must describe exactly one workspace, project, or content root",
                path.display()
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
struct WorkspaceMetadata {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ProjectMetadata {
    kind: ProjectKind,
    id: String,
}

#[derive(Debug, Deserialize)]
struct ContentMetadata {
    id: String,
}

fn collect_content(
    entities: &mut Vec<VaporEntity>,
    metadata: Option<ContentMetadata>,
    kind: ContentKind,
    path: &Path,
) -> Result<(), String> {
    if let Some(metadata) = metadata {
        entities.push(VaporEntity::Content {
            kind,
            id: validate(metadata.id, "content id", path)?,
        });
    }
    Ok(())
}

fn validate(value: String, label: &str, path: &Path) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(format!("{label} in '{}' cannot be empty", path.display()))
    } else {
        Ok(trimmed.to_owned())
    }
}
