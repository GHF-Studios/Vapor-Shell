//! Workshop/content discovery, packaging, installation state, and receipts.
//!
//! The module keeps SteamUGC and filesystem staging behind Vapor content
//! operations. Authored intent is read from source `Vapor.toml` files; generated
//! app-owned state is written under the Steam installation/app root.

use crate::{
    discovery::{EnvironmentPaths, InstallationPaths, ensure_contained},
    manifest::{self, ContentKind, VaporEntity},
    steam,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const CONTENT_STATE_SCHEMA: u32 = 1;
const FINGERPRINT_ALGORITHM: &str = "vapor-fnv1a-64-v1";
const PACKAGE_MANIFEST: &str = "Vapor-package.toml";

/// Canonical app-root layout for generated content state.
#[derive(Debug, Clone)]
pub struct ContentLayout {
    app_content: PathBuf,
    steam_downloads: PathBuf,
    cache: PathBuf,
    installed: PathBuf,
    disabled: PathBuf,
    quarantine: PathBuf,
    output_packages: PathBuf,
    output_scripts: PathBuf,
    state: PathBuf,
    locks: PathBuf,
    receipts: PathBuf,
    selection: PathBuf,
    index: PathBuf,
}

impl ContentLayout {
    /// Build layout paths under the Steam installation/app root.
    pub fn new(installation: &InstallationPaths) -> Self {
        let app_content = installation.root().join("content");
        let state = installation.state_dir().join("content");
        Self {
            steam_downloads: app_content.join("workshop/downloads"),
            cache: app_content.join("cache"),
            installed: app_content.join("installed"),
            disabled: app_content.join("disabled"),
            quarantine: app_content.join("quarantine"),
            output_packages: installation.root().join("output/content/packages"),
            output_scripts: installation.root().join("output/content/scripts"),
            app_content,
            locks: state.join("locks"),
            receipts: state.join("receipts"),
            selection: state.join("selection.toml"),
            index: state.join("index.toml"),
            state,
        }
    }

    /// Runtime content root.
    pub fn app_content(&self) -> &Path {
        &self.app_content
    }

    /// Provider-observed Steam Workshop download records.
    pub fn steam_downloads(&self) -> &Path {
        &self.steam_downloads
    }

    /// Vapor-managed package cache root.
    pub fn cache(&self) -> &Path {
        &self.cache
    }

    /// Active installed content root.
    pub fn installed(&self) -> &Path {
        &self.installed
    }

    /// Disabled but retained content root.
    pub fn disabled(&self) -> &Path {
        &self.disabled
    }

    /// Corrupted or incomplete content quarantine root.
    pub fn quarantine(&self) -> &Path {
        &self.quarantine
    }

    /// Content package staging output.
    pub fn output_packages(&self) -> &Path {
        &self.output_packages
    }

    /// Steam/Workshop script preview output.
    pub fn output_scripts(&self) -> &Path {
        &self.output_scripts
    }

    /// Generated content state root.
    pub fn state(&self) -> &Path {
        &self.state
    }

    /// Generated resolution lock directory.
    pub fn locks(&self) -> &Path {
        &self.locks
    }

    /// Operation receipt directory.
    pub fn receipts(&self) -> &Path {
        &self.receipts
    }

    /// Selected packagepack state path.
    pub fn selection(&self) -> &Path {
        &self.selection
    }

    /// Installed-content index path.
    pub fn index(&self) -> &Path {
        &self.index
    }
}

/// Source-discovered content artifact catalog.
#[derive(Debug, Clone)]
pub struct ContentCatalog {
    artifacts: Vec<ContentArtifact>,
}

impl ContentCatalog {
    /// Discovered publishable or installable source artifacts.
    pub fn artifacts(&self) -> &[ContentArtifact] {
        &self.artifacts
    }

    /// Find an artifact by full ID, local name, or published Workshop ID.
    pub fn find(&self, selector: &str) -> Option<&ContentArtifact> {
        self.artifacts.iter().find(|artifact| {
            artifact.id == selector
                || artifact.name == selector
                || artifact
                    .workshop
                    .published_file_id
                    .as_deref()
                    .is_some_and(|id| id == selector)
        })
    }

    fn by_id(&self, id: &str) -> Option<&ContentArtifact> {
        self.artifacts.iter().find(|artifact| artifact.id == id)
    }
}

/// One source-authored Vapor content artifact.
#[derive(Debug, Clone)]
pub struct ContentArtifact {
    id: String,
    name: String,
    kind: ContentKind,
    root: PathBuf,
    manifest: PathBuf,
    version: Option<String>,
    dependencies: Vec<ContentReference>,
    conflicts: Vec<ContentReference>,
    workshop: WorkshopPolicy,
}

impl ContentArtifact {
    /// Fully-qualified content ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Local artifact name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Canonical content kind.
    pub fn kind(&self) -> ContentKind {
        self.kind
    }

    /// Source directory containing the artifact manifest.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Source artifact manifest path.
    pub fn manifest(&self) -> &Path {
        &self.manifest
    }

    /// Resolved artifact version, when declared.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Required or optional content dependencies and composition edges.
    pub fn dependencies(&self) -> &[ContentReference] {
        &self.dependencies
    }

    /// Declared content conflicts.
    pub fn conflicts(&self) -> &[ContentReference] {
        &self.conflicts
    }

    /// Authored Workshop publication policy.
    pub fn workshop(&self) -> &WorkshopPolicy {
        &self.workshop
    }
}

/// Authored dependency, conflict, or composition reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentReference {
    id: String,
    #[serde(default = "default_reference_relationship")]
    relationship: String,
    #[serde(default)]
    optional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workshop_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

fn default_reference_relationship() -> String {
    "dependency".to_owned()
}

impl ContentReference {
    /// Referenced artifact ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Relationship label, such as `dependency`, `engine`, or `conflict`.
    pub fn relationship(&self) -> &str {
        &self.relationship
    }

    /// Whether missing referenced content is acceptable.
    pub fn optional(&self) -> bool {
        self.optional
    }

    /// Required version expression, when declared.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Referenced Workshop item ID, when declared.
    pub fn workshop_id(&self) -> Option<&str> {
        self.workshop_id.as_deref()
    }

    /// Human-readable reason, when declared.
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// Authored Workshop publication policy for one artifact.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WorkshopPolicy {
    #[serde(skip_serializing_if = "Option::is_none")]
    app_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    published_file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    visibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    change_note: Option<String>,
}

impl WorkshopPolicy {
    /// Steam AppID targeted by this Workshop item.
    pub fn app_id(&self) -> Option<u32> {
        self.app_id
    }

    /// PublishedFileId for an existing Workshop item.
    pub fn published_file_id(&self) -> Option<&str> {
        self.published_file_id.as_deref()
    }

    /// Authored visibility string.
    pub fn visibility(&self) -> Option<&str> {
        self.visibility.as_deref()
    }

    /// Authored Workshop title.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Authored Workshop description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Authored Workshop tags.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Default update note.
    pub fn change_note(&self) -> Option<&str> {
        self.change_note.as_deref()
    }
}

/// Stable tree fingerprint used by packages, locks, and receipts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fingerprint {
    algorithm: String,
    hash: String,
    files: u64,
    bytes: u64,
}

impl Fingerprint {
    /// Fingerprint algorithm identifier.
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    /// Hex-encoded fingerprint hash.
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// Number of regular files included.
    pub fn files(&self) -> u64 {
        self.files
    }

    /// Number of file-content bytes included.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Package staging result.
#[derive(Debug, Clone)]
pub struct PackageReport {
    artifact_id: String,
    root: PathBuf,
    payload: PathBuf,
    fingerprint: Fingerprint,
    receipt: Option<PathBuf>,
    dry_run: bool,
}

impl PackageReport {
    /// Packaged artifact ID.
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Package root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Package payload root.
    pub fn payload(&self) -> &Path {
        &self.payload
    }

    /// Payload fingerprint.
    pub fn fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }

    /// Operation receipt path, absent for dry-runs.
    pub fn receipt(&self) -> Option<&Path> {
        self.receipt.as_deref()
    }

    /// Whether the package was only previewed.
    pub fn dry_run(&self) -> bool {
        self.dry_run
    }
}

/// Validation result for one or more source artifacts.
#[derive(Debug, Clone)]
pub struct ValidationReport {
    checked: Vec<String>,
    diagnostics: Vec<String>,
}

impl ValidationReport {
    /// Artifact IDs that were checked.
    pub fn checked(&self) -> &[String] {
        &self.checked
    }

    /// Non-fatal validation diagnostics.
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
}

/// Result of acquiring content into the app-owned cache.
#[derive(Debug, Clone)]
pub struct AcquireReport {
    artifact_id: String,
    cache_root: PathBuf,
    fingerprint: Fingerprint,
    receipt: PathBuf,
}

impl AcquireReport {
    /// Acquired artifact ID.
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Cache root containing the package.
    pub fn cache_root(&self) -> &Path {
        &self.cache_root
    }

    /// Cached payload fingerprint.
    pub fn fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }

    /// Operation receipt path.
    pub fn receipt(&self) -> &Path {
        &self.receipt
    }
}

/// Result of installing or updating content.
#[derive(Debug, Clone)]
pub struct InstallReport {
    artifact_id: String,
    installed_root: PathBuf,
    fingerprint: Fingerprint,
    receipt: PathBuf,
}

impl InstallReport {
    /// Installed artifact ID.
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Installed payload root.
    pub fn installed_root(&self) -> &Path {
        &self.installed_root
    }

    /// Installed payload fingerprint.
    pub fn fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }

    /// Operation receipt path.
    pub fn receipt(&self) -> &Path {
        &self.receipt
    }
}

/// Result of uninstalling content.
#[derive(Debug, Clone)]
pub struct UninstallReport {
    artifact_id: String,
    removed: bool,
    receipt: PathBuf,
}

impl UninstallReport {
    /// Uninstalled artifact ID.
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Whether an installed or disabled payload was removed.
    pub fn removed(&self) -> bool {
        self.removed
    }

    /// Operation receipt path.
    pub fn receipt(&self) -> &Path {
        &self.receipt
    }
}

/// Selected installed packagepack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagepackSelection {
    schema_version: u32,
    artifact_id: String,
    installed_root: PathBuf,
    fingerprint: Fingerprint,
    selected_at: u64,
}

impl PackagepackSelection {
    /// Selected packagepack artifact ID.
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Installed payload root selected for play.
    pub fn installed_root(&self) -> &Path {
        &self.installed_root
    }

    /// Fingerprint selected at the time of selection.
    pub fn fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }
}

/// Verification result for one installed artifact.
#[derive(Debug, Clone)]
pub struct VerifyReport {
    artifact_id: String,
    ok: bool,
    expected: Option<Fingerprint>,
    observed: Option<Fingerprint>,
    detail: String,
}

impl VerifyReport {
    /// Verified artifact ID.
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Whether the installed payload matches its receipt/index fingerprint.
    pub fn ok(&self) -> bool {
        self.ok
    }

    /// Expected fingerprint from app-owned state.
    pub fn expected(&self) -> Option<&Fingerprint> {
        self.expected.as_ref()
    }

    /// Observed fingerprint from the installed payload.
    pub fn observed(&self) -> Option<&Fingerprint> {
        self.observed.as_ref()
    }

    /// Diagnostic detail.
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// Publish, create, or delete preview/execution result.
#[derive(Debug, Clone)]
pub struct WorkshopOperationReport {
    artifact_id: String,
    script: Option<PathBuf>,
    receipt: PathBuf,
    uploaded: bool,
}

impl WorkshopOperationReport {
    /// Target artifact or Workshop ID.
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Generated provider script, when one exists.
    pub fn script(&self) -> Option<&Path> {
        self.script.as_deref()
    }

    /// Operation receipt path.
    pub fn receipt(&self) -> &Path {
        &self.receipt
    }

    /// Whether a provider upload was attempted and accepted.
    pub fn uploaded(&self) -> bool {
        self.uploaded
    }
}

/// Discover source-authored content artifacts in the active workspace.
///
/// # Errors
///
/// Returns filesystem, TOML, or manifest validation diagnostics.
pub fn discover(paths: &EnvironmentPaths) -> Result<ContentCatalog, String> {
    let source_root = paths.source().root();
    let workspace_version = workspace_version(source_root)?;
    let mut manifests = Vec::new();
    collect_manifest_paths(source_root, source_root, &mut manifests)?;
    manifests.sort();

    let mut artifacts = Vec::new();
    for manifest_path in manifests {
        if manifest_path == source_root.join(manifest::FILE_NAME) {
            continue;
        }
        match manifest::read(&manifest_path, source_root)? {
            VaporEntity::Content { kind, id, name } => {
                let root = manifest_path
                    .parent()
                    .ok_or_else(|| format!("manifest has no parent: {}", manifest_path.display()))?
                    .to_path_buf();
                let authored = read_authored_content(&manifest_path, kind)?;
                artifacts.push(ContentArtifact {
                    id,
                    name,
                    kind,
                    root,
                    manifest: manifest_path,
                    version: authored.version.resolve(workspace_version.as_deref()),
                    dependencies: authored.dependencies,
                    conflicts: authored.conflicts,
                    workshop: authored.workshop,
                });
            }
            VaporEntity::Project { .. } => {}
            VaporEntity::Root { id, .. } | VaporEntity::Workspace { id, .. } => {
                if manifest_path != source_root.join(manifest::FILE_NAME) {
                    return Err(format!(
                        "nested source root '{id}' is not valid content in '{}'",
                        manifest_path.display()
                    ));
                }
            }
            VaporEntity::Registry { id, .. } => {
                return Err(format!(
                    "registry '{id}' cannot be nested in content source '{}'",
                    manifest_path.display()
                ));
            }
        }
    }
    artifacts.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(ContentCatalog { artifacts })
}

/// Validate content dependency, conflict, and Workshop metadata.
///
/// # Errors
///
/// Returns an error when required references or publication fields are invalid.
pub fn validate(
    paths: &EnvironmentPaths,
    selector: Option<&str>,
) -> Result<ValidationReport, String> {
    let catalog = discover(paths)?;
    let selected = select_artifacts(&catalog, selector)?;
    let ids = catalog
        .artifacts()
        .iter()
        .map(|artifact| artifact.id().to_owned())
        .collect::<BTreeSet<_>>();
    let mut diagnostics = Vec::new();
    let mut errors = Vec::new();

    for artifact in &selected {
        for dependency in artifact.dependencies() {
            if !dependency.optional()
                && !ids.contains(dependency.id())
                && dependency.workshop_id().is_none()
            {
                errors.push(format!(
                    "{} requires missing {} reference '{}'",
                    artifact.id(),
                    dependency.relationship(),
                    dependency.id()
                ));
            }
            if dependency.optional() && !ids.contains(dependency.id()) {
                diagnostics.push(format!(
                    "{} has optional {} reference '{}' outside this workspace",
                    artifact.id(),
                    dependency.relationship(),
                    dependency.id()
                ));
            }
        }
        for conflict in artifact.conflicts() {
            if conflict.id() == artifact.id() {
                errors.push(format!("{} conflicts with itself", artifact.id()));
            }
        }
        if artifact.workshop().app_id().is_none() {
            diagnostics.push(format!(
                "{} has no [{}.steam].app-id; local lifecycle is available but Workshop publish is not",
                artifact.id(),
                artifact.kind()
            ));
        }
    }

    if errors.is_empty() {
        Ok(ValidationReport {
            checked: selected
                .iter()
                .map(|artifact| artifact.id().to_owned())
                .collect(),
            diagnostics,
        })
    } else {
        Err(errors.join("\n"))
    }
}

/// Stage a source content artifact as a Vapor package.
///
/// # Errors
///
/// Returns discovery, validation, or filesystem errors.
pub fn package(
    paths: &EnvironmentPaths,
    selector: &str,
    dry_run: bool,
) -> Result<PackageReport, String> {
    let catalog = discover(paths)?;
    let artifact = catalog.find(selector).ok_or_else(|| {
        format!(
            "unknown content artifact '{selector}'\nhelp: inspect source content with `content list`"
        )
    })?;
    validate(paths, Some(artifact.id()))?;
    let layout = ContentLayout::new(paths.installation());
    let package_root = layout.output_packages().join(slug(artifact.id()));
    let payload_root = package_root.join("payload");
    let fingerprint = fingerprint_tree(artifact.root())?;

    if dry_run {
        return Ok(PackageReport {
            artifact_id: artifact.id().to_owned(),
            root: package_root,
            payload: payload_root,
            fingerprint,
            receipt: None,
            dry_run: true,
        });
    }

    reset_directory(paths.installation().root(), &package_root)?;
    fs::create_dir_all(&payload_root).map_err(io("create package payload", &payload_root))?;
    copy_tree(
        artifact.root(),
        &payload_root,
        artifact.root(),
        default_exclusions(),
    )?;
    let fingerprint = fingerprint_tree(&payload_root)?;
    write_package_manifest(&package_root, artifact, &fingerprint)?;
    let receipt = write_receipt(
        paths.installation(),
        "package",
        artifact.id(),
        "staged",
        Some(&format!("package={}", package_root.display())),
    )?;
    Ok(PackageReport {
        artifact_id: artifact.id().to_owned(),
        root: package_root,
        payload: payload_root,
        fingerprint,
        receipt: Some(receipt),
        dry_run: false,
    })
}

/// Acquire content into the app-owned cache.
///
/// Local source artifacts are packaged and copied into the cache. Numeric
/// Workshop IDs install from an existing cache until a live SteamUGC provider is
/// available in the running app.
///
/// # Errors
///
/// Returns provider, discovery, package, or filesystem errors.
pub fn acquire(
    installation: &InstallationPaths,
    paths: Option<&EnvironmentPaths>,
    selector: &str,
) -> Result<AcquireReport, String> {
    let layout = ContentLayout::new(installation);
    if let Some(paths) = paths {
        let catalog = discover(paths)?;
        if let Some(artifact) = catalog.find(selector) {
            let package = package(paths, artifact.id(), false)?;
            let cache_key = artifact
                .workshop()
                .published_file_id()
                .map_or_else(|| slug(artifact.id()), ToOwned::to_owned);
            let cache_root = layout.cache().join("packages").join(cache_key);
            reset_directory(installation.root(), &cache_root)?;
            copy_tree(package.root(), &cache_root, package.root(), &[])?;
            let receipt = write_receipt(
                installation,
                "acquire",
                artifact.id(),
                "cached",
                Some(&format!("cache={}", cache_root.display())),
            )?;
            let mut index = load_index(&layout)?;
            index.caches.insert(
                artifact.id().to_owned(),
                CacheRecord {
                    artifact_id: artifact.id().to_owned(),
                    workshop_id: artifact
                        .workshop()
                        .published_file_id()
                        .map(ToOwned::to_owned),
                    cache_root: cache_root.clone(),
                    fingerprint: package.fingerprint().clone(),
                    acquired_at: now_seconds(),
                },
            );
            save_index(&layout, &index)?;
            return Ok(AcquireReport {
                artifact_id: artifact.id().to_owned(),
                cache_root,
                fingerprint: package.fingerprint().clone(),
                receipt,
            });
        }
    }

    let index = load_index(&layout)?;
    if let Some((artifact_id, cache)) = index.cache_by_selector(selector) {
        let receipt = write_receipt(
            installation,
            "acquire",
            &artifact_id,
            "cached-already",
            Some(&format!("cache={}", cache.cache_root.display())),
        )?;
        return Ok(AcquireReport {
            artifact_id,
            cache_root: cache.cache_root.clone(),
            fingerprint: cache.fingerprint.clone(),
            receipt,
        });
    }

    Err(format!(
        "cannot acquire Workshop item '{selector}': no cached package exists and the live SteamUGC download provider is not available in this build\nhelp: acquire from an open source artifact first, or run inside a SteamUGC-enabled Vapor session"
    ))
}

/// Install source or cached content into the app-owned installed-content tree.
///
/// # Errors
///
/// Returns dependency, conflict, provider, verification, or filesystem errors.
pub fn install(
    installation: &InstallationPaths,
    paths: Option<&EnvironmentPaths>,
    selector: &str,
) -> Result<Vec<InstallReport>, String> {
    let layout = ContentLayout::new(installation);
    let catalog = paths.map(discover).transpose()?;
    let mut index = load_index(&layout)?;
    let mut reports = Vec::new();
    let mut visiting = BTreeSet::new();
    install_selector(
        installation,
        paths,
        catalog.as_ref(),
        selector,
        &layout,
        &mut index,
        &mut visiting,
        &mut reports,
    )?;
    save_index(&layout, &index)?;
    Ok(reports)
}

/// Update installed content by reinstalling it from source or cache.
///
/// # Errors
///
/// Returns the same errors as [`install`].
pub fn update(
    installation: &InstallationPaths,
    paths: Option<&EnvironmentPaths>,
    selector: Option<&str>,
) -> Result<Vec<InstallReport>, String> {
    let layout = ContentLayout::new(installation);
    let index = load_index(&layout)?;
    let targets = if let Some(selector) = selector {
        vec![resolve_installed_selector(&index, selector)?]
    } else {
        index.installed.keys().cloned().collect()
    };
    if targets.is_empty() {
        return Err("no installed content to update".to_owned());
    }
    let mut reports = Vec::new();
    for target in targets {
        reports.extend(install(installation, paths, &target)?);
    }
    Ok(reports)
}

/// Disable installed content without deleting its payload.
///
/// # Errors
///
/// Returns an error when the target is unknown or the move fails.
pub fn disable(installation: &InstallationPaths, selector: &str) -> Result<InstallReport, String> {
    move_enabled_state(installation, selector, false)
}

/// Enable disabled content without reinstalling it.
///
/// # Errors
///
/// Returns an error when the target is unknown or the move fails.
pub fn enable(installation: &InstallationPaths, selector: &str) -> Result<InstallReport, String> {
    move_enabled_state(installation, selector, true)
}

/// Uninstall content and remove generated installed-state records.
///
/// # Errors
///
/// Returns an error when generated state cannot be updated.
pub fn uninstall(
    installation: &InstallationPaths,
    selector: &str,
) -> Result<UninstallReport, String> {
    let layout = ContentLayout::new(installation);
    let mut index = load_index(&layout)?;
    let artifact_id = resolve_installed_selector(&index, selector)?;
    let mut removed = false;
    for root in [
        layout.installed().join(&artifact_id),
        layout.disabled().join(&artifact_id),
    ] {
        if root.exists() {
            remove_directory(installation.root(), &root)?;
            removed = true;
        }
    }
    index.installed.remove(&artifact_id);
    save_index(&layout, &index)?;
    if current_selection(installation)?
        .as_ref()
        .is_some_and(|selection| selection.artifact_id() == artifact_id)
    {
        clear_selection(installation)?;
    }
    let receipt = write_receipt(
        installation,
        "uninstall",
        &artifact_id,
        if removed { "removed" } else { "not-present" },
        None,
    )?;
    Ok(UninstallReport {
        artifact_id,
        removed,
        receipt,
    })
}

/// Select an installed packagepack for play.
///
/// # Errors
///
/// Returns an error when the target is not installed, not enabled, not a
/// packagepack, or the selection cannot be written.
pub fn select_packagepack(
    installation: &InstallationPaths,
    selector: &str,
) -> Result<PackagepackSelection, String> {
    let layout = ContentLayout::new(installation);
    let index = load_index(&layout)?;
    let artifact_id = resolve_installed_selector(&index, selector)?;
    let record = index
        .installed
        .get(&artifact_id)
        .ok_or_else(|| format!("content is not installed: {artifact_id}"))?;
    if record.kind != ContentKind::Packagepack.to_string() {
        return Err(format!(
            "selected content must be a packagepack: {} is {}",
            artifact_id, record.kind
        ));
    }
    if !record.enabled {
        return Err(format!("cannot select disabled packagepack: {artifact_id}"));
    }
    if !record.installed_root.exists() {
        return Err(format!(
            "cannot select packagepack with missing payload: {}",
            record.installed_root.display()
        ));
    }
    let selection = PackagepackSelection {
        schema_version: CONTENT_STATE_SCHEMA,
        artifact_id: artifact_id.clone(),
        installed_root: record.installed_root.clone(),
        fingerprint: record.fingerprint.clone(),
        selected_at: now_seconds(),
    };
    if let Some(parent) = layout.selection().parent() {
        fs::create_dir_all(parent).map_err(io("create content selection parent", parent))?;
    }
    let encoded = toml::to_string_pretty(&selection)
        .map_err(|error| format!("failed to encode packagepack selection: {error}"))?;
    fs::write(layout.selection(), encoded)
        .map_err(io("write packagepack selection", layout.selection()))?;
    write_receipt(
        installation,
        "select",
        &artifact_id,
        "selected",
        Some(&format!("root={}", selection.installed_root.display())),
    )?;
    Ok(selection)
}

/// Read the current selected packagepack.
///
/// # Errors
///
/// Returns filesystem or TOML parse errors.
pub fn current_selection(
    installation: &InstallationPaths,
) -> Result<Option<PackagepackSelection>, String> {
    let layout = ContentLayout::new(installation);
    if !layout.selection().is_file() {
        return Ok(None);
    }
    let source = fs::read_to_string(layout.selection())
        .map_err(io("read packagepack selection", layout.selection()))?;
    toml::from_str(&source).map(Some).map_err(|error| {
        format!(
            "failed to parse '{}': {error}",
            layout.selection().display()
        )
    })
}

/// Clear the selected packagepack.
///
/// # Errors
///
/// Returns filesystem errors.
pub fn clear_selection(installation: &InstallationPaths) -> Result<(), String> {
    let layout = ContentLayout::new(installation);
    if layout.selection().exists() {
        fs::remove_file(layout.selection())
            .map_err(io("remove packagepack selection", layout.selection()))?;
    }
    write_receipt(installation, "deselect", "packagepack", "cleared", None)?;
    Ok(())
}

/// Verify installed content against app-owned receipts and indexes.
///
/// # Errors
///
/// Returns an error when state cannot be read.
pub fn verify(
    installation: &InstallationPaths,
    selector: Option<&str>,
) -> Result<Vec<VerifyReport>, String> {
    let layout = ContentLayout::new(installation);
    let index = load_index(&layout)?;
    let targets = if let Some(selector) = selector {
        vec![resolve_installed_selector(&index, selector)?]
    } else {
        index.installed.keys().cloned().collect()
    };
    Ok(targets
        .into_iter()
        .map(|artifact_id| verify_one(&index, &artifact_id))
        .collect())
}

/// Repair corrupted installed content by reinstalling from source or cache.
///
/// # Errors
///
/// Returns provider or filesystem errors when repair cannot proceed.
pub fn repair(
    installation: &InstallationPaths,
    paths: Option<&EnvironmentPaths>,
    selector: Option<&str>,
) -> Result<Vec<InstallReport>, String> {
    let layout = ContentLayout::new(installation);
    let reports = verify(installation, selector)?;
    let mut repaired = Vec::new();
    for report in reports {
        if report.ok() {
            continue;
        }
        let corrupt = layout.installed().join(report.artifact_id());
        if corrupt.exists() {
            let quarantine = layout.quarantine().join(format!(
                "{}-{}",
                slug(report.artifact_id()),
                now_seconds()
            ));
            if let Some(parent) = quarantine.parent() {
                fs::create_dir_all(parent).map_err(io("create quarantine parent", parent))?;
            }
            fs::rename(&corrupt, &quarantine).map_err(io("quarantine content", &corrupt))?;
            write_receipt(
                installation,
                "repair",
                report.artifact_id(),
                "quarantined",
                Some(&format!("quarantine={}", quarantine.display())),
            )?;
        }
        repaired.extend(install(installation, paths, report.artifact_id())?);
    }
    if repaired.is_empty() {
        let target = selector.unwrap_or("all");
        write_receipt(installation, "repair", target, "already-ok", None)?;
    }
    Ok(repaired)
}

/// Write a Workshop create preview receipt.
///
/// Real item creation requires a live SteamUGC provider and remains a manual
/// authority-changing operation.
///
/// # Errors
///
/// Returns discovery or receipt errors.
pub fn create_workshop_item(
    paths: &EnvironmentPaths,
    selector: &str,
    dry_run: bool,
) -> Result<WorkshopOperationReport, String> {
    let catalog = discover(paths)?;
    let artifact = catalog.find(selector).ok_or_else(|| {
        format!("unknown content artifact '{selector}'\nhelp: inspect source content with `content list`")
    })?;
    if !dry_run {
        return Err(
            "real Workshop item creation requires the controlled SteamUGC provider and manual confirmation; this build can only preview creation"
                .to_owned(),
        );
    }
    let receipt = write_receipt(
        paths.installation(),
        "workshop-create",
        artifact.id(),
        "dry-run",
        Some("would request a new PublishedFileId from SteamUGC"),
    )?;
    Ok(WorkshopOperationReport {
        artifact_id: artifact.id().to_owned(),
        script: None,
        receipt,
        uploaded: false,
    })
}

/// Publish or preview a Workshop item update.
///
/// Dry-runs write the package and SteamCMD Workshop VDF. Real updates require
/// `account` and `confirmed` and use the controlled SteamCMD provider.
///
/// # Errors
///
/// Returns package, provider, SteamCMD, or authority errors.
pub fn publish_workshop_item(
    paths: &EnvironmentPaths,
    selector: &str,
    account: Option<&str>,
    change_note: Option<&str>,
    dry_run: bool,
    confirmed: bool,
) -> Result<WorkshopOperationReport, String> {
    let catalog = discover(paths)?;
    let artifact = catalog.find(selector).ok_or_else(|| {
        format!("unknown content artifact '{selector}'\nhelp: inspect source content with `content list`")
    })?;
    if artifact.workshop().app_id().is_none() {
        return Err(format!(
            "{} has no [{}.steam].app-id and cannot be published",
            artifact.id(),
            artifact.kind()
        ));
    }
    if artifact.workshop().published_file_id().is_none() && !dry_run {
        return Err(format!(
            "{} has no PublishedFileId; run `content create {} --dry-run` and create the item manually before a real update",
            artifact.id(),
            artifact.name()
        ));
    }
    if !dry_run && account.unwrap_or("").trim().is_empty() {
        return Err(
            "real Workshop publication requires --account ACCOUNT after reviewing --dry-run"
                .to_owned(),
        );
    }
    if !dry_run && !confirmed {
        return Err(
            "real Workshop publication requires --yes after reviewing --dry-run".to_owned(),
        );
    }

    let package = package(paths, artifact.id(), false)?;
    let script = write_workshop_script(paths, artifact, package.payload(), change_note, dry_run)?;
    let receipt_status = if dry_run { "dry-run" } else { "uploaded" };
    let mut uploaded = false;
    if !dry_run {
        let steamcmd = steam::executable(paths)?;
        let status = Command::new(&steamcmd)
            .args(["+login", account.expect("account checked")])
            .arg("+workshop_build_item")
            .arg(&script)
            .arg("+quit")
            .current_dir(steamcmd.parent().expect("SteamCMD has a parent"))
            .status()
            .map_err(|error| format!("failed to start SteamCMD: {error}"))?;
        if !status.success() {
            return Err(format!("Steam Workshop publish exited with {status}"));
        }
        uploaded = true;
    }
    let receipt = write_receipt(
        paths.installation(),
        "workshop-publish",
        artifact.id(),
        receipt_status,
        Some(&format!("script={}", script.display())),
    )?;
    Ok(WorkshopOperationReport {
        artifact_id: artifact.id().to_owned(),
        script: Some(script),
        receipt,
        uploaded,
    })
}

/// Preview a Workshop delete operation.
///
/// Deletion needs a live SteamUGC provider; this build records the authority
/// boundary and refuses real deletion.
///
/// # Errors
///
/// Returns receipt errors or a provider diagnostic for real deletion.
pub fn delete_workshop_item(
    installation: &InstallationPaths,
    selector: &str,
    dry_run: bool,
    confirmed: bool,
) -> Result<WorkshopOperationReport, String> {
    if !dry_run && !confirmed {
        return Err("real Workshop deletion requires --yes after reviewing --dry-run".to_owned());
    }
    if !dry_run {
        return Err(
            "real Workshop deletion requires the controlled SteamUGC provider; this build can only preview deletion"
                .to_owned(),
        );
    }
    let receipt = write_receipt(
        installation,
        "workshop-delete",
        selector,
        "dry-run",
        Some("would delete or retire the PublishedFileId through SteamUGC"),
    )?;
    Ok(WorkshopOperationReport {
        artifact_id: selector.to_owned(),
        script: None,
        receipt,
        uploaded: false,
    })
}

/// Read the app-owned content index.
///
/// # Errors
///
/// Returns filesystem or TOML parse errors.
pub fn installed_index(installation: &InstallationPaths) -> Result<Vec<String>, String> {
    let layout = ContentLayout::new(installation);
    let index = load_index(&layout)?;
    Ok(index.installed.keys().cloned().collect())
}

fn install_selector(
    installation: &InstallationPaths,
    paths: Option<&EnvironmentPaths>,
    catalog: Option<&ContentCatalog>,
    selector: &str,
    layout: &ContentLayout,
    index: &mut ContentIndex,
    visiting: &mut BTreeSet<String>,
    reports: &mut Vec<InstallReport>,
) -> Result<(), String> {
    if let Some(catalog) = catalog {
        if let Some(artifact) = catalog.find(selector) {
            if !visiting.insert(artifact.id().to_owned()) {
                return Err(format!(
                    "content dependency cycle includes {}",
                    artifact.id()
                ));
            }
            for dependency in artifact
                .dependencies()
                .iter()
                .filter(|item| !item.optional())
            {
                if catalog.by_id(dependency.id()).is_some() {
                    install_selector(
                        installation,
                        paths,
                        catalog.into(),
                        dependency.id(),
                        layout,
                        index,
                        visiting,
                        reports,
                    )?;
                } else if !index.installed.contains_key(dependency.id()) {
                    return Err(format!(
                        "{} requires missing {} '{}'",
                        artifact.id(),
                        dependency.relationship(),
                        dependency.id()
                    ));
                }
            }
            for conflict in artifact.conflicts() {
                if index
                    .installed
                    .get(conflict.id())
                    .is_some_and(|record| record.enabled)
                {
                    return Err(format!(
                        "{} conflicts with installed content '{}'",
                        artifact.id(),
                        conflict.id()
                    ));
                }
            }
            let package = package(
                paths.expect("source catalog only exists with source paths"),
                artifact.id(),
                false,
            )?;
            let report = install_package(
                installation,
                layout,
                index,
                artifact,
                package.payload(),
                package.fingerprint().clone(),
                "source-package",
            )?;
            reports.push(report);
            visiting.remove(artifact.id());
            return Ok(());
        }
    }

    let cache = index
        .cache_by_selector(selector)
        .map(|(_, cache)| cache.clone())
        .ok_or_else(|| {
            format!(
                "cannot install '{selector}': no matching source artifact or app-owned cache entry exists\nhelp: run `content acquire ARTIFACT` from an open source, or acquire through a SteamUGC-enabled session"
            )
        })?;
    let manifest = read_package_manifest(&cache.cache_root)?;
    let payload = cache.cache_root.join("payload");
    let pseudo = manifest.into_artifact(PathBuf::new(), payload.clone());
    let report = install_package(
        installation,
        layout,
        index,
        &pseudo,
        &payload,
        cache.fingerprint,
        "cache",
    )?;
    reports.push(report);
    Ok(())
}

fn install_package(
    installation: &InstallationPaths,
    layout: &ContentLayout,
    index: &mut ContentIndex,
    artifact: &ContentArtifact,
    payload: &Path,
    expected_fingerprint: Fingerprint,
    source: &str,
) -> Result<InstallReport, String> {
    let target = layout.installed().join(artifact.id());
    reset_directory(installation.root(), &target)?;
    fs::create_dir_all(&target).map_err(io("create installed content", &target))?;
    copy_tree(payload, &target, payload, &[])?;
    let fingerprint = fingerprint_tree(&target)?;
    if fingerprint != expected_fingerprint {
        return Err(format!(
            "installed fingerprint mismatch for {}\n  expected: {}\n  observed: {}",
            artifact.id(),
            expected_fingerprint.hash(),
            fingerprint.hash()
        ));
    }
    let record = InstalledRecord {
        artifact_id: artifact.id().to_owned(),
        kind: artifact.kind().to_string(),
        version: artifact.version().map(ToOwned::to_owned),
        workshop_id: artifact
            .workshop()
            .published_file_id()
            .map(ToOwned::to_owned),
        enabled: true,
        installed_root: target.clone(),
        fingerprint: fingerprint.clone(),
        dependencies: artifact.dependencies().to_vec(),
        conflicts: artifact.conflicts().to_vec(),
        installed_at: now_seconds(),
        source: source.to_owned(),
    };
    index
        .installed
        .insert(artifact.id().to_owned(), record.clone());
    write_lock(layout, &record)?;
    let receipt = write_receipt(
        installation,
        "install",
        artifact.id(),
        "installed",
        Some(&format!("root={}", target.display())),
    )?;
    Ok(InstallReport {
        artifact_id: artifact.id().to_owned(),
        installed_root: target,
        fingerprint,
        receipt,
    })
}

fn move_enabled_state(
    installation: &InstallationPaths,
    selector: &str,
    enable: bool,
) -> Result<InstallReport, String> {
    let layout = ContentLayout::new(installation);
    let mut index = load_index(&layout)?;
    let artifact_id = resolve_installed_selector(&index, selector)?;
    let record = index
        .installed
        .get_mut(&artifact_id)
        .ok_or_else(|| format!("content is not installed: {artifact_id}"))?;
    let (from_root, to_root) = if enable {
        (
            layout.disabled().join(&artifact_id),
            layout.installed().join(&artifact_id),
        )
    } else {
        (
            layout.installed().join(&artifact_id),
            layout.disabled().join(&artifact_id),
        )
    };
    if !from_root.exists() {
        return Err(format!(
            "cannot {} {}: expected payload is missing at {}",
            if enable { "enable" } else { "disable" },
            artifact_id,
            from_root.display()
        ));
    }
    if let Some(parent) = to_root.parent() {
        fs::create_dir_all(parent).map_err(io("create content state parent", parent))?;
    }
    if to_root.exists() {
        remove_directory(installation.root(), &to_root)?;
    }
    fs::rename(&from_root, &to_root).map_err(io("move content payload", &from_root))?;
    let fingerprint = fingerprint_tree(&to_root)?;
    record.enabled = enable;
    record.installed_root = to_root.clone();
    record.fingerprint = fingerprint.clone();
    let record_clone = record.clone();
    save_index(&layout, &index)?;
    write_lock(&layout, &record_clone)?;
    let receipt = write_receipt(
        installation,
        if enable { "enable" } else { "disable" },
        &artifact_id,
        if enable { "enabled" } else { "disabled" },
        Some(&format!("root={}", to_root.display())),
    )?;
    Ok(InstallReport {
        artifact_id,
        installed_root: to_root,
        fingerprint,
        receipt,
    })
}

fn verify_one(index: &ContentIndex, artifact_id: &str) -> VerifyReport {
    let Some(record) = index.installed.get(artifact_id) else {
        return VerifyReport {
            artifact_id: artifact_id.to_owned(),
            ok: false,
            expected: None,
            observed: None,
            detail: "not installed".to_owned(),
        };
    };
    if !record.installed_root.exists() {
        return VerifyReport {
            artifact_id: artifact_id.to_owned(),
            ok: false,
            expected: Some(record.fingerprint.clone()),
            observed: None,
            detail: format!(
                "installed payload is missing: {}",
                record.installed_root.display()
            ),
        };
    }
    match fingerprint_tree(&record.installed_root) {
        Ok(observed) if observed == record.fingerprint => VerifyReport {
            artifact_id: artifact_id.to_owned(),
            ok: true,
            expected: Some(record.fingerprint.clone()),
            observed: Some(observed),
            detail: "fingerprint matches".to_owned(),
        },
        Ok(observed) => VerifyReport {
            artifact_id: artifact_id.to_owned(),
            ok: false,
            expected: Some(record.fingerprint.clone()),
            observed: Some(observed),
            detail: "fingerprint mismatch".to_owned(),
        },
        Err(error) => VerifyReport {
            artifact_id: artifact_id.to_owned(),
            ok: false,
            expected: Some(record.fingerprint.clone()),
            observed: None,
            detail: error,
        },
    }
}

fn select_artifacts<'a>(
    catalog: &'a ContentCatalog,
    selector: Option<&str>,
) -> Result<Vec<&'a ContentArtifact>, String> {
    if let Some(selector) = selector {
        catalog.find(selector).map(|artifact| vec![artifact]).ok_or_else(|| {
            format!(
                "unknown content artifact '{selector}'\nhelp: inspect source content with `content list`"
            )
        })
    } else {
        Ok(catalog.artifacts().iter().collect())
    }
}

fn collect_manifest_paths(
    root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let relative = directory.strip_prefix(root).unwrap_or(Path::new(""));
    if is_ignored_directory(relative) {
        return Ok(());
    }
    for entry in fs::read_dir(directory).map_err(io("read source directory", directory))? {
        let entry =
            entry.map_err(|error| format!("failed to read source directory entry: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect '{}': {error}", path.display()))?;
        if file_type.is_dir() {
            collect_manifest_paths(root, &path, output)?;
        } else if file_type.is_file() && entry.file_name() == manifest::FILE_NAME {
            output.push(path);
        }
    }
    Ok(())
}

fn is_ignored_directory(relative: &Path) -> bool {
    relative.components().any(|component| {
        let Component::Normal(name) = component else {
            return false;
        };
        matches!(
            name.to_str(),
            Some(".git" | "target" | "output" | ".idea" | ".vapor")
        )
    })
}

fn workspace_version(root: &Path) -> Result<Option<String>, String> {
    let path = root.join(manifest::FILE_NAME);
    let source = fs::read_to_string(&path).map_err(io("read source manifest", &path))?;
    #[derive(Deserialize)]
    struct Root {
        workspace: Option<WorkspaceMeta>,
    }
    #[derive(Deserialize)]
    struct WorkspaceMeta {
        version: Option<String>,
    }
    let parsed: Root = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", path.display()))?;
    Ok(parsed.workspace.and_then(|workspace| workspace.version))
}

fn read_authored_content(path: &Path, kind: ContentKind) -> Result<AuthoredContent, String> {
    let source = fs::read_to_string(path).map_err(io("read content manifest", path))?;
    let parsed: AuthoredManifest = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", path.display()))?;
    let mut content = parsed.content(kind).ok_or_else(|| {
        format!(
            "content manifest '{}' did not contain expected [{kind}] section",
            path.display()
        )
    })?;
    content
        .dependencies
        .extend(composition_edges(kind, &content));
    Ok(content)
}

fn composition_edges(kind: ContentKind, content: &AuthoredContent) -> Vec<ContentReference> {
    match kind {
        ContentKind::Packagepack => {
            let mut edges = Vec::new();
            if let Some(reference) = &content.engine {
                edges.push(reference.to_content_reference("engine", false));
            }
            if let Some(reference) = &content.enginepack {
                edges.push(reference.to_content_reference("enginepack", false));
            }
            if let Some(reference) = &content.game {
                edges.push(reference.to_content_reference("game", false));
            }
            if let Some(reference) = &content.gamepack {
                edges.push(reference.to_content_reference("gamepack", false));
            }
            edges
        }
        _ => Vec::new(),
    }
}

#[derive(Debug, Deserialize)]
struct AuthoredManifest {
    engine: Option<AuthoredContent>,
    game: Option<AuthoredContent>,
    packagepack: Option<AuthoredContent>,
    enginepack: Option<AuthoredContent>,
    gamepack: Option<AuthoredContent>,
    modpack: Option<AuthoredContent>,
    #[serde(rename = "engine-mod")]
    engine_mod: Option<AuthoredContent>,
    #[serde(rename = "game-mod")]
    game_mod: Option<AuthoredContent>,
    #[serde(rename = "extension-mod")]
    extension_mod: Option<AuthoredContent>,
}

impl AuthoredManifest {
    fn content(self, kind: ContentKind) -> Option<AuthoredContent> {
        match kind {
            ContentKind::Engine => self.engine,
            ContentKind::Game => self.game,
            ContentKind::Packagepack => self.packagepack,
            ContentKind::Enginepack => self.enginepack,
            ContentKind::Gamepack => self.gamepack,
            ContentKind::Modpack => self.modpack,
            ContentKind::EngineMod => self.engine_mod,
            ContentKind::GameMod => self.game_mod,
            ContentKind::ExtensionMod => self.extension_mod,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct AuthoredContent {
    #[serde(default)]
    version: AuthoredVersion,
    #[serde(default, rename = "steam")]
    workshop: WorkshopPolicy,
    #[serde(default)]
    dependencies: Vec<ContentReference>,
    #[serde(default)]
    conflicts: Vec<ContentReference>,
    engine: Option<AuthoredReference>,
    enginepack: Option<AuthoredReference>,
    game: Option<AuthoredReference>,
    gamepack: Option<AuthoredReference>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(untagged)]
enum AuthoredVersion {
    Exact(String),
    Workspace {
        workspace: bool,
    },
    #[default]
    Missing,
}

impl AuthoredVersion {
    fn resolve(&self, workspace_version: Option<&str>) -> Option<String> {
        match self {
            Self::Exact(version) => Some(version.clone()),
            Self::Workspace { workspace: true } => workspace_version.map(ToOwned::to_owned),
            Self::Workspace { workspace: false } | Self::Missing => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct AuthoredReference {
    id: String,
    #[serde(default)]
    optional: bool,
    version: Option<String>,
    workshop_id: Option<String>,
    reason: Option<String>,
}

impl AuthoredReference {
    fn to_content_reference(&self, relationship: &str, default_optional: bool) -> ContentReference {
        ContentReference {
            id: self.id.clone(),
            relationship: relationship.to_owned(),
            optional: self.optional || default_optional,
            version: self.version.clone(),
            workshop_id: self.workshop_id.clone(),
            reason: self.reason.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContentPackageManifest {
    schema_version: u32,
    artifact_id: String,
    name: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<ContentReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    conflicts: Vec<ContentReference>,
    #[serde(default)]
    workshop: WorkshopPolicy,
    fingerprint: Fingerprint,
}

impl ContentPackageManifest {
    fn into_artifact(self, root: PathBuf, manifest: PathBuf) -> ContentArtifact {
        ContentArtifact {
            id: self.artifact_id,
            name: self.name,
            kind: parse_kind(&self.kind).unwrap_or(ContentKind::Packagepack),
            root,
            manifest,
            version: self.version,
            dependencies: self.dependencies,
            conflicts: self.conflicts,
            workshop: self.workshop,
        }
    }
}

fn write_package_manifest(
    package_root: &Path,
    artifact: &ContentArtifact,
    fingerprint: &Fingerprint,
) -> Result<(), String> {
    let manifest = ContentPackageManifest {
        schema_version: CONTENT_STATE_SCHEMA,
        artifact_id: artifact.id().to_owned(),
        name: artifact.name().to_owned(),
        kind: artifact.kind().to_string(),
        version: artifact.version().map(ToOwned::to_owned),
        dependencies: artifact.dependencies().to_vec(),
        conflicts: artifact.conflicts().to_vec(),
        workshop: artifact.workshop().clone(),
        fingerprint: fingerprint.clone(),
    };
    let encoded = toml::to_string_pretty(&manifest)
        .map_err(|error| format!("failed to encode package manifest: {error}"))?;
    let path = package_root.join(PACKAGE_MANIFEST);
    fs::write(&path, encoded).map_err(io("write package manifest", &path))
}

fn read_package_manifest(package_root: &Path) -> Result<ContentPackageManifest, String> {
    let path = package_root.join(PACKAGE_MANIFEST);
    let source = fs::read_to_string(&path).map_err(io("read package manifest", &path))?;
    toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", path.display()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledRecord {
    artifact_id: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workshop_id: Option<String>,
    enabled: bool,
    installed_root: PathBuf,
    fingerprint: Fingerprint,
    #[serde(default)]
    dependencies: Vec<ContentReference>,
    #[serde(default)]
    conflicts: Vec<ContentReference>,
    installed_at: u64,
    source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheRecord {
    artifact_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    workshop_id: Option<String>,
    cache_root: PathBuf,
    fingerprint: Fingerprint,
    acquired_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContentIndex {
    schema_version: u32,
    #[serde(default)]
    installed: BTreeMap<String, InstalledRecord>,
    #[serde(default)]
    caches: BTreeMap<String, CacheRecord>,
}

impl Default for ContentIndex {
    fn default() -> Self {
        Self {
            schema_version: CONTENT_STATE_SCHEMA,
            installed: BTreeMap::new(),
            caches: BTreeMap::new(),
        }
    }
}

impl ContentIndex {
    fn cache_by_selector(&self, selector: &str) -> Option<(String, &CacheRecord)> {
        self.caches.iter().find_map(|(id, cache)| {
            (id == selector
                || cache.artifact_id == selector
                || cache
                    .workshop_id
                    .as_deref()
                    .is_some_and(|workshop_id| workshop_id == selector))
            .then(|| (id.clone(), cache))
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OperationReceipt {
    schema_version: u32,
    action: String,
    artifact_id: String,
    status: String,
    timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

fn load_index(layout: &ContentLayout) -> Result<ContentIndex, String> {
    if !layout.index().is_file() {
        return Ok(ContentIndex::default());
    }
    let source =
        fs::read_to_string(layout.index()).map_err(io("read content index", layout.index()))?;
    let mut index: ContentIndex = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", layout.index().display()))?;
    if index.schema_version == 0 {
        index.schema_version = CONTENT_STATE_SCHEMA;
    }
    Ok(index)
}

fn save_index(layout: &ContentLayout, index: &ContentIndex) -> Result<(), String> {
    if let Some(parent) = layout.index().parent() {
        fs::create_dir_all(parent).map_err(io("create content index parent", parent))?;
    }
    let encoded = toml::to_string_pretty(index)
        .map_err(|error| format!("failed to encode content index: {error}"))?;
    fs::write(layout.index(), encoded).map_err(io("write content index", layout.index()))
}

fn write_lock(layout: &ContentLayout, record: &InstalledRecord) -> Result<(), String> {
    fs::create_dir_all(layout.locks()).map_err(io("create content locks", layout.locks()))?;
    let path = layout
        .locks()
        .join(format!("{}.toml", slug(&record.artifact_id)));
    let encoded = toml::to_string_pretty(record)
        .map_err(|error| format!("failed to encode content lock: {error}"))?;
    fs::write(&path, encoded).map_err(io("write content lock", &path))
}

fn write_receipt(
    installation: &InstallationPaths,
    action: &str,
    artifact_id: &str,
    status: &str,
    detail: Option<&str>,
) -> Result<PathBuf, String> {
    let layout = ContentLayout::new(installation);
    fs::create_dir_all(layout.receipts())
        .map_err(io("create content receipts", layout.receipts()))?;
    let receipt = OperationReceipt {
        schema_version: CONTENT_STATE_SCHEMA,
        action: action.to_owned(),
        artifact_id: artifact_id.to_owned(),
        status: status.to_owned(),
        timestamp: now_seconds(),
        detail: detail.map(ToOwned::to_owned),
    };
    let path = layout.receipts().join(format!(
        "{}-{}-{}.toml",
        receipt.timestamp,
        action,
        slug(artifact_id)
    ));
    let encoded = toml::to_string_pretty(&receipt)
        .map_err(|error| format!("failed to encode content receipt: {error}"))?;
    fs::write(&path, encoded).map_err(io("write content receipt", &path))?;
    Ok(path)
}

fn resolve_installed_selector(index: &ContentIndex, selector: &str) -> Result<String, String> {
    index
        .installed
        .iter()
        .find_map(|(id, record)| {
            (id == selector
                || record.artifact_id == selector
                || record
                    .workshop_id
                    .as_deref()
                    .is_some_and(|workshop_id| workshop_id == selector))
            .then(|| id.clone())
        })
        .ok_or_else(|| format!("content is not installed: {selector}"))
}

fn write_workshop_script(
    paths: &EnvironmentPaths,
    artifact: &ContentArtifact,
    payload: &Path,
    change_note: Option<&str>,
    preview: bool,
) -> Result<PathBuf, String> {
    let layout = ContentLayout::new(paths.installation());
    fs::create_dir_all(layout.output_scripts())
        .map_err(io("create Workshop script output", layout.output_scripts()))?;
    let script = layout
        .output_scripts()
        .join(format!("workshop_{}.vdf", slug(artifact.id())));
    let policy = artifact.workshop();
    let preview_line = if preview {
        "    \"preview\" \"1\"\n"
    } else {
        ""
    };
    let mut tags = String::new();
    tags.push_str("    \"tags\"\n    {\n");
    let mut all_tags = vec![artifact.kind().to_string()];
    all_tags.extend(policy.tags().iter().cloned());
    for (index, tag) in all_tags.iter().enumerate() {
        tags.push_str(&format!(
            "        \"{}\" \"{}\"\n",
            index,
            steam_escape(tag)
        ));
    }
    tags.push_str("    }\n");
    let vdf = format!(
        "\"workshopitem\"\n{{\n    \"appid\" \"{}\"\n{}    \"publishedfileid\" \"{}\"\n    \"contentfolder\" \"{}\"\n    \"visibility\" \"{}\"\n    \"title\" \"{}\"\n    \"description\" \"{}\"\n    \"changenote\" \"{}\"\n{tags}}}\n",
        policy.app_id().expect("app id checked"),
        preview_line,
        steam_escape(policy.published_file_id().unwrap_or("0")),
        steam_escape(&payload.display().to_string()),
        steam_escape(policy.visibility().unwrap_or("private")),
        steam_escape(policy.title().unwrap_or(artifact.name())),
        steam_escape(policy.description().unwrap_or("")),
        steam_escape(
            change_note
                .or_else(|| policy.change_note())
                .unwrap_or("Vapor content update")
        ),
    );
    fs::write(&script, vdf).map_err(io("write Workshop script", &script))?;
    Ok(script)
}

fn fingerprint_tree(root: &Path) -> Result<Fingerprint, String> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort();
    let mut hash = 0xcbf29ce484222325u64;
    let mut file_count = 0u64;
    let mut byte_count = 0u64;
    for relative in files {
        let path = root.join(&relative);
        update_hash(&mut hash, relative.to_string_lossy().as_bytes());
        update_hash(&mut hash, &[0]);
        let mut file = fs::File::open(&path).map_err(io("open fingerprint input", &path))?;
        let mut buffer = [0u8; 8192];
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(io("read fingerprint input", &path))?;
            if read == 0 {
                break;
            }
            update_hash(&mut hash, &buffer[..read]);
            byte_count += read as u64;
        }
        file_count += 1;
    }
    Ok(Fingerprint {
        algorithm: FINGERPRINT_ALGORITHM.to_owned(),
        hash: format!("{hash:016x}"),
        files: file_count,
        bytes: byte_count,
    })
}

fn collect_files(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(directory).map_err(io("read fingerprint directory", directory))? {
        let entry = entry.map_err(|error| format!("failed to read fingerprint entry: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect '{}': {error}", path.display()))?;
        if file_type.is_dir() {
            collect_files(root, &path, output)?;
        } else if file_type.is_file() {
            output.push(
                path.strip_prefix(root)
                    .map_err(|error| format!("failed to relativize '{}': {error}", path.display()))?
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

fn update_hash(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

fn copy_tree(source: &Path, target: &Path, root: &Path, exclusions: &[&str]) -> Result<(), String> {
    let relative = source.strip_prefix(root).unwrap_or(Path::new(""));
    if exclusions
        .iter()
        .any(|excluded| relative.starts_with(Path::new(excluded)))
    {
        return Ok(());
    }
    let metadata = fs::metadata(source).map_err(io("inspect copy source", source))?;
    if metadata.is_dir() {
        fs::create_dir_all(target).map_err(io("create copy target", target))?;
        for entry in fs::read_dir(source).map_err(io("read copy source", source))? {
            let entry = entry.map_err(|error| format!("failed to read copy entry: {error}"))?;
            copy_tree(
                &entry.path(),
                &target.join(entry.file_name()),
                root,
                exclusions,
            )?;
        }
    } else if metadata.is_file() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io("create copy target parent", parent))?;
        }
        fs::copy(source, target).map_err(io("copy content file", source))?;
    }
    Ok(())
}

fn default_exclusions() -> &'static [&'static str] {
    &[".git", "target", "output"]
}

fn reset_directory(app_root: &Path, target: &Path) -> Result<(), String> {
    ensure_contained(app_root, target)?;
    if target.exists() {
        remove_directory(app_root, target)?;
    }
    fs::create_dir_all(target).map_err(io("create directory", target))
}

fn remove_directory(app_root: &Path, target: &Path) -> Result<(), String> {
    ensure_contained(app_root, target)?;
    fs::remove_dir_all(target).map_err(io("remove directory", target))
}

fn slug(id: &str) -> String {
    id.chars()
        .map(|character| {
            if character.is_ascii_lowercase()
                || character.is_ascii_uppercase()
                || character.is_ascii_digit()
                || character == '-'
            {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn parse_kind(kind: &str) -> Option<ContentKind> {
    match kind {
        "engine" => Some(ContentKind::Engine),
        "game" => Some(ContentKind::Game),
        "packagepack" => Some(ContentKind::Packagepack),
        "enginepack" => Some(ContentKind::Enginepack),
        "gamepack" => Some(ContentKind::Gamepack),
        "modpack" => Some(ContentKind::Modpack),
        "engine-mod" => Some(ContentKind::EngineMod),
        "game-mod" => Some(ContentKind::GameMod),
        "extension-mod" => Some(ContentKind::ExtensionMod),
        _ => None,
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn steam_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn io<'a>(action: &'a str, path: &'a Path) -> impl Fn(std::io::Error) -> String + 'a {
    move |error| format!("failed to {action} '{}': {error}", path.display())
}
