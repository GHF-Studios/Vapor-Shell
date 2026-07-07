//! Declarative assembly of the self-hosting Steam application payload.

use crate::discovery::{EnvironmentPaths, ensure_contained};
use serde::Deserialize;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

/// Distribution manifest filename at the source root.
pub const FILE_NAME: &str = "Vapor.toml";

/// Parsed application and payload policy.
#[derive(Debug, Clone, Deserialize)]
pub struct DistributionManifest {
    #[serde(skip)]
    _private: (),
    application: Application,
    #[serde(default = "default_payload")]
    payload: Vec<Payload>,
}

/// Steam application identifiers and development branch.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Application {
    app_id: u32,
    depot_id: u32,
    development_branch: String,
}

/// One allowlisted staging input.
#[derive(Debug, Clone, Deserialize)]
pub struct Payload {
    root: PayloadRoot,
    from: PathBuf,
    to: PathBuf,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    exclude: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum PayloadRoot {
    Installation,
    Source,
}

/// Completed staging summary.
#[derive(Debug, Clone)]
pub struct StageReport {
    root: PathBuf,
    files: usize,
}

impl DistributionManifest {
    /// Load and validate the umbrella distribution manifest.
    pub fn load(paths: &EnvironmentPaths) -> Result<Self, String> {
        Self::load_optional(paths)?.ok_or_else(|| {
            format!(
                "source root '{}' does not declare [root.steam]",
                paths.source().root().display()
            )
        })
    }

    /// Load the root Steam policy when one is declared.
    ///
    /// A source root does not need to be a self-hosting Steam application,
    /// so absence is distinct from malformed distribution policy.
    pub fn load_optional(paths: &EnvironmentPaths) -> Result<Option<Self>, String> {
        let path = paths.source().root().join(FILE_NAME);
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
        #[derive(Deserialize)]
        struct Root {
            root: Option<RootPolicy>,
        }
        #[derive(Deserialize)]
        struct RootPolicy {
            steam: Option<Application>,
        }
        let manifest = toml::from_str::<Root>(&text)
            .map_err(|error| format!("failed to parse '{}': {error}", path.display()))?;
        let Some(root) = manifest.root else {
            return Ok(None);
        };
        let Some(application) = root.steam else {
            return Ok(None);
        };
        let manifest = DistributionManifest {
            _private: (),
            application,
            payload: default_payload(),
        };
        if manifest.application.app_id == 0 || manifest.application.depot_id == 0 {
            return Err("Steam AppID and DepotID must be non-zero".to_owned());
        }
        if manifest.application.development_branch.trim().is_empty()
            || manifest.application.development_branch == "default"
        {
            return Err("development branch must be non-empty and non-default".to_owned());
        }
        for item in &manifest.payload {
            validate_relative(&item.from)?;
            validate_relative(&item.to)?;
            for exclusion in &item.exclude {
                validate_relative(exclusion)?;
            }
        }
        Ok(Some(manifest))
    }

    /// Steam application policy.
    pub fn application(&self) -> &Application {
        &self.application
    }
}

fn default_payload() -> Vec<Payload> {
    vec![
        Payload::required(PayloadRoot::Installation, "Vapor.toml", "Vapor.toml"),
        Payload::required(PayloadRoot::Installation, "bin", "bin"),
        Payload::required(PayloadRoot::Installation, "docs", "docs"),
        Payload::required(
            PayloadRoot::Installation,
            "packages/toolchain",
            "packages/toolchain",
        ),
    ]
}

impl Payload {
    fn required(root: PayloadRoot, from: &str, to: &str) -> Self {
        Self {
            root,
            from: PathBuf::from(from),
            to: PathBuf::from(to),
            required: true,
            exclude: Vec::new(),
        }
    }
}

impl Application {
    /// Steam AppID.
    pub fn app_id(&self) -> u32 {
        self.app_id
    }
    /// Steam DepotID.
    pub fn depot_id(&self) -> u32 {
        self.depot_id
    }
    /// Automatically activated development beta branch.
    pub fn development_branch(&self) -> &str {
        &self.development_branch
    }
}

impl StageReport {
    /// Staged depot content root.
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Number of copied files.
    pub fn files(&self) -> usize {
        self.files
    }
}

/// Rebuild the clean, allowlisted Steam depot staging tree.
pub fn stage(
    paths: &EnvironmentPaths,
    manifest: &DistributionManifest,
) -> Result<StageReport, String> {
    let root = paths.installation().root().join("output/root/content");
    ensure_contained(paths.installation().root(), &root)?;
    if root.exists() {
        fs::remove_dir_all(&root).map_err(io("reset staging", &root))?;
    }
    fs::create_dir_all(&root).map_err(io("create staging", &root))?;

    let mut files = 0;
    for item in &manifest.payload {
        let base = match item.root {
            PayloadRoot::Installation => paths.installation().root(),
            PayloadRoot::Source => paths.source().root(),
        };
        let source = base.join(&item.from);
        if !source.exists() {
            if item.required {
                return Err(format!("required payload is missing: {}", source.display()));
            }
            continue;
        }
        let canonical = fs::canonicalize(&source).map_err(io("resolve payload", &source))?;
        ensure_contained(base, &canonical)?;
        files += copy_tree(&canonical, &root.join(&item.to), &canonical, &item.exclude)?;
    }
    Ok(StageReport { root, files })
}

fn copy_tree(
    source: &Path,
    target: &Path,
    item_root: &Path,
    exclusions: &[PathBuf],
) -> Result<usize, String> {
    let relative = source.strip_prefix(item_root).unwrap_or(Path::new(""));
    if exclusions
        .iter()
        .any(|excluded| relative.starts_with(excluded))
    {
        return Ok(0);
    }
    let canonical = fs::canonicalize(source).map_err(io("resolve payload entry", source))?;
    ensure_contained(item_root, &canonical)?;
    let metadata = fs::metadata(&canonical).map_err(io("inspect payload", &canonical))?;
    if metadata.is_dir() {
        fs::create_dir_all(target).map_err(io("create payload directory", target))?;
        let mut files = 0;
        for entry in fs::read_dir(&canonical).map_err(io("read payload directory", &canonical))? {
            let entry = entry.map_err(|error| format!("failed to read payload entry: {error}"))?;
            files += copy_tree(
                &entry.path(),
                &target.join(entry.file_name()),
                item_root,
                exclusions,
            )?;
        }
        Ok(files)
    } else if metadata.is_file() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io("create payload parent", parent))?;
        }
        fs::copy(&canonical, target).map_err(io("copy payload", &canonical))?;
        Ok(1)
    } else {
        Ok(0)
    }
}

fn validate_relative(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        Err(format!(
            "distribution path must be safe and relative: {}",
            path.display()
        ))
    } else {
        Ok(())
    }
}

fn io<'a>(action: &'a str, path: &'a Path) -> impl Fn(std::io::Error) -> String + 'a {
    move |error| format!("failed to {action} '{}': {error}", path.display())
}
