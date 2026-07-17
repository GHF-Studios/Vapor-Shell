//! Workshop/content discovery, packaging, installation state, and receipts.
//!
//! The module keeps SteamUGC and filesystem staging behind Vapor content
//! operations. Authored intent is read from source `Vapor.toml` files; generated
//! app-owned state is written under the Steam installation/app root.

use crate::{
    discovery::{EnvironmentPaths, InstallationPaths, ensure_contained},
    manifest::{self, ContentKind, VaporEntity},
    steam,
    workspace::WorkspaceManifest,
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
    binaries: Vec<String>,
    libraries: Vec<String>,
    runtime: Vec<RuntimePayload>,
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

    /// Runtime executables copied from app-local Cargo output into `bin/`.
    pub fn binaries(&self) -> &[String] {
        &self.binaries
    }

    /// Runtime libraries copied from app-local Cargo output into `lib/`.
    pub fn libraries(&self) -> &[String] {
        &self.libraries
    }

    /// Target-specific deployed runtime payloads.
    fn runtime(&self) -> &[RuntimePayload] {
        &self.runtime
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RuntimePayload {
    target: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    binaries: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    libraries: Vec<String>,
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

    fn is_empty(&self) -> bool {
        self.app_id.is_none()
            && self.published_file_id.is_none()
            && self.visibility.is_none()
            && self.title.is_none()
            && self.description.is_none()
            && self.tags.is_empty()
            && self.change_note.is_none()
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
    fingerprint: Fingerprint,
    receipt: Option<PathBuf>,
    dry_run: bool,
    runtime_target: String,
}

impl PackageReport {
    /// Packaged artifact ID.
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Staged artifact root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Staged artifact fingerprint.
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

    /// Runtime target triple used for copied binaries and libraries.
    pub fn runtime_target(&self) -> &str {
        &self.runtime_target
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

    /// Cached artifact-root fingerprint.
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

    /// Installed artifact root.
    pub fn installed_root(&self) -> &Path {
        &self.installed_root
    }

    /// Installed artifact-root fingerprint.
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

    /// Whether an installed or disabled artifact root was removed.
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

    /// Installed artifact root selected for play.
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

    /// Whether the installed artifact root matches its receipt/index fingerprint.
    pub fn ok(&self) -> bool {
        self.ok
    }

    /// Expected fingerprint from app-owned state.
    pub fn expected(&self) -> Option<&Fingerprint> {
        self.expected.as_ref()
    }

    /// Observed fingerprint from the installed artifact root.
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
    published_file_id: Option<String>,
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

    /// PublishedFileId created or used by the provider, when known.
    pub fn published_file_id(&self) -> Option<&str> {
        self.published_file_id.as_deref()
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
    let workspace = WorkspaceManifest::load(paths)?;

    let mut artifacts = Vec::new();
    for project in workspace.projects() {
        let Some(kind) = project.kind().content_kind() else {
            continue;
        };
        let manifest_path = source_root.join(project.manifest());
        match manifest::read(&manifest_path, source_root)? {
            VaporEntity::Content {
                kind: actual_kind,
                id,
                name,
            } => {
                if actual_kind != kind {
                    return Err(format!(
                        "registered workspace project '{}' changed kind from {kind} to {actual_kind}",
                        project.path().display()
                    ));
                }
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
                    binaries: authored.binaries,
                    libraries: authored.libraries,
                    runtime: Vec::new(),
                    dependencies: authored.dependencies,
                    conflicts: authored.conflicts,
                    workshop: authored.workshop,
                });
            }
            VaporEntity::Project { .. } => {}
            VaporEntity::Root { id, .. } | VaporEntity::Workspace { id, .. } => {
                return Err(format!(
                    "registered workspace project '{}' declares nested source root '{id}'",
                    project.path().display()
                ));
            }
            VaporEntity::Registry { id, .. } => {
                return Err(format!(
                    "registered workspace project '{}' declares registry '{id}'",
                    project.path().display()
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
    package_for_target(paths, selector, dry_run, None)
}

/// Stage a source content artifact for one runtime target.
///
/// # Errors
///
/// Returns discovery, validation, runtime-output, or filesystem errors.
pub fn package_for_target(
    paths: &EnvironmentPaths,
    selector: &str,
    dry_run: bool,
    target_triple: Option<&str>,
) -> Result<PackageReport, String> {
    let targets = target_triple
        .map(|target| vec![target.to_owned()])
        .unwrap_or_default();
    package_for_targets(paths, selector, dry_run, &targets)
}

/// Stage a source content artifact for one or more runtime targets.
///
/// An empty target list uses the host target and Cargo's host-default output
/// directory. Non-empty target lists expect Cargo's target-specific output
/// directories.
///
/// # Errors
///
/// Returns discovery, validation, runtime-output, or filesystem errors.
pub fn package_for_targets(
    paths: &EnvironmentPaths,
    selector: &str,
    dry_run: bool,
    target_triples: &[String],
) -> Result<PackageReport, String> {
    let runtime_targets = RuntimeTarget::many(target_triples)?;
    package_with_runtime_targets(paths, selector, dry_run, runtime_targets)
}

fn package_with_runtime_targets(
    paths: &EnvironmentPaths,
    selector: &str,
    dry_run: bool,
    runtime_targets: Vec<RuntimeTarget>,
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
    let runtime_target_label = runtime_target_label(&runtime_targets);

    if dry_run {
        let temporary_root = TemporaryDirectory::new("vapor-content-package", artifact.id())?;
        stage_deployed_artifact(paths, temporary_root.path(), artifact, &runtime_targets)?;
        let fingerprint = fingerprint_tree(temporary_root.path())?;
        return Ok(PackageReport {
            artifact_id: artifact.id().to_owned(),
            root: package_root,
            fingerprint,
            receipt: None,
            dry_run: true,
            runtime_target: runtime_target_label,
        });
    }

    reset_directory(paths.installation().root(), &package_root)?;
    stage_deployed_artifact(paths, &package_root, artifact, &runtime_targets)?;
    let fingerprint = fingerprint_tree(&package_root)?;
    let receipt = write_receipt(
        paths.installation(),
        "package",
        artifact.id(),
        "staged",
        Some(&format!(
            "package={};target={runtime_target_label}",
            package_root.display()
        )),
    )?;
    Ok(PackageReport {
        artifact_id: artifact.id().to_owned(),
        root: package_root,
        fingerprint,
        receipt: Some(receipt),
        dry_run: false,
        runtime_target: runtime_target_label,
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
    account: Option<&str>,
) -> Result<AcquireReport, String> {
    let layout = ContentLayout::new(installation);
    if is_published_file_id(selector) && account.is_some_and(|value| !value.trim().is_empty()) {
        let app_id = resolve_download_app_id(installation, paths, selector)?;
        return acquire_workshop_item(installation, app_id, selector, account);
    }

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

    if let Some(seed) = root_content_seed(installation, selector)? {
        return acquire_workshop_item(installation, seed.app_id, &seed.workshop_id, account);
    }

    if is_published_file_id(selector) {
        let app_id = resolve_download_app_id(installation, paths, selector)?;
        return acquire_workshop_item(installation, app_id, selector, account);
    }

    Err(format!(
        "cannot acquire Workshop item '{selector}': no matching source artifact, app-owned cache entry, root content seed, or numeric PublishedFileId exists\nhelp: acquire from an open source artifact, add a [[root.content]] seed, or pass a PublishedFileId"
    ))
}

/// Acquire one or more content items into the app-owned cache.
///
/// Numeric PublishedFileIds are downloaded through one SteamCMD provider
/// session. Mixed source/cache selectors fall back to the single-item
/// acquisition path.
///
/// # Errors
///
/// Returns provider, discovery, package, or filesystem errors.
pub fn acquire_many(
    installation: &InstallationPaths,
    paths: Option<&EnvironmentPaths>,
    selectors: &[String],
    account: Option<&str>,
) -> Result<Vec<AcquireReport>, String> {
    if selectors.is_empty() {
        return Err("at least one content target is required".to_owned());
    }
    if selectors
        .iter()
        .all(|selector| is_published_file_id(selector))
    {
        let downloads = selectors
            .iter()
            .map(|selector| {
                Ok(WorkshopDownload {
                    app_id: resolve_download_app_id(installation, paths, selector)?,
                    published_file_id: selector.to_owned(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        return acquire_workshop_items(installation, &downloads, account);
    }
    selectors
        .iter()
        .map(|selector| acquire(installation, paths, selector, account))
        .collect()
}

fn acquire_workshop_item(
    installation: &InstallationPaths,
    app_id: u32,
    published_file_id: &str,
    account: Option<&str>,
) -> Result<AcquireReport, String> {
    let account = workshop_download_account(account);
    let provider_root =
        run_steamcmd_workshop_download(installation, app_id, published_file_id, &account)?;
    import_workshop_item(installation, app_id, published_file_id, &provider_root)
}

fn acquire_workshop_items(
    installation: &InstallationPaths,
    downloads: &[WorkshopDownload],
    account: Option<&str>,
) -> Result<Vec<AcquireReport>, String> {
    let account = workshop_download_account(account);
    let provider_roots = run_steamcmd_workshop_downloads(installation, downloads, &account)?;
    downloads
        .iter()
        .zip(provider_roots.iter())
        .map(|(download, provider_root)| {
            import_workshop_item(
                installation,
                download.app_id,
                &download.published_file_id,
                provider_root,
            )
        })
        .collect()
}

fn workshop_download_account(account: Option<&str>) -> String {
    let account = account.unwrap_or("").trim();
    if account.is_empty() {
        "anonymous".to_owned()
    } else {
        account.to_owned()
    }
}

fn import_workshop_item(
    installation: &InstallationPaths,
    app_id: u32,
    published_file_id: &str,
    provider_root: &Path,
) -> Result<AcquireReport, String> {
    let layout = ContentLayout::new(installation);
    let observed_root = layout
        .steam_downloads()
        .join(app_id.to_string())
        .join(published_file_id);
    reset_directory(installation.root(), &observed_root)?;
    copy_tree(provider_root, &observed_root, provider_root, &[])?;
    let artifact = read_deployed_manifest(&observed_root)?;
    let cache_root = layout.cache().join("packages").join(published_file_id);
    reset_directory(installation.root(), &cache_root)?;
    copy_tree(&observed_root, &cache_root, &observed_root, &[])?;
    let fingerprint = fingerprint_tree(&cache_root)?;
    let mut index = load_index(&layout)?;
    index.caches.insert(
        artifact.id().to_owned(),
        CacheRecord {
            artifact_id: artifact.id().to_owned(),
            workshop_id: Some(published_file_id.to_owned()),
            cache_root: cache_root.clone(),
            fingerprint: fingerprint.clone(),
            acquired_at: now_seconds(),
        },
    );
    save_index(&layout, &index)?;
    let receipt = write_receipt(
        installation,
        "download",
        artifact.id(),
        "downloaded",
        Some(&format!(
            "workshop-id={published_file_id}; observed={}; cache={}",
            observed_root.display(),
            cache_root.display()
        )),
    )?;
    Ok(AcquireReport {
        artifact_id: artifact.id().to_owned(),
        cache_root,
        fingerprint,
        receipt,
    })
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
    install_with_account(installation, paths, selector, None)
}

/// Install source or cached content for one runtime target.
///
/// # Errors
///
/// Returns dependency, conflict, provider, verification, or filesystem errors.
pub fn install_for_target(
    installation: &InstallationPaths,
    paths: Option<&EnvironmentPaths>,
    selector: &str,
    target_triple: Option<&str>,
) -> Result<Vec<InstallReport>, String> {
    install_with_account_for_target(installation, paths, selector, None, target_triple)
}

/// Install source or cached content for one or more runtime targets.
///
/// # Errors
///
/// Returns dependency, conflict, provider, verification, or filesystem errors.
pub fn install_for_targets(
    installation: &InstallationPaths,
    paths: Option<&EnvironmentPaths>,
    selector: &str,
    target_triples: &[String],
) -> Result<Vec<InstallReport>, String> {
    install_with_account_for_targets(installation, paths, selector, None, target_triples)
}

/// Install source or cached content, downloading Workshop dependencies with an account when needed.
///
/// # Errors
///
/// Returns dependency, conflict, provider, verification, or filesystem errors.
pub fn install_with_account(
    installation: &InstallationPaths,
    paths: Option<&EnvironmentPaths>,
    selector: &str,
    account: Option<&str>,
) -> Result<Vec<InstallReport>, String> {
    install_with_account_for_target(installation, paths, selector, account, None)
}

/// Install source or cached content with an explicit runtime target.
///
/// # Errors
///
/// Returns dependency, conflict, provider, verification, or filesystem errors.
pub fn install_with_account_for_target(
    installation: &InstallationPaths,
    paths: Option<&EnvironmentPaths>,
    selector: &str,
    account: Option<&str>,
    target_triple: Option<&str>,
) -> Result<Vec<InstallReport>, String> {
    let targets = target_triple
        .map(|target| vec![target.to_owned()])
        .unwrap_or_default();
    install_with_account_for_targets(installation, paths, selector, account, &targets)
}

/// Install source or cached content with explicit runtime targets.
///
/// # Errors
///
/// Returns dependency, conflict, provider, verification, or filesystem errors.
pub fn install_with_account_for_targets(
    installation: &InstallationPaths,
    paths: Option<&EnvironmentPaths>,
    selector: &str,
    account: Option<&str>,
    target_triples: &[String],
) -> Result<Vec<InstallReport>, String> {
    let runtime_targets = RuntimeTarget::many(target_triples)?;
    let layout = ContentLayout::new(installation);
    let catalog = paths.map(discover).transpose()?;
    let mut index = load_index(&layout)?;
    let mut reports = Vec::new();
    let mut visiting = BTreeSet::new();
    let context = InstallContext {
        installation,
        paths,
        catalog: catalog.as_ref(),
        layout: &layout,
        account,
        runtime_targets: &runtime_targets,
    };
    install_selector(&context, selector, &mut index, &mut visiting, &mut reports)?;
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

/// Disable installed content without deleting its artifact root.
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
            "cannot select packagepack with missing artifact root: {}",
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
    account: Option<&str>,
    dry_run: bool,
    confirmed: bool,
) -> Result<WorkshopOperationReport, String> {
    create_workshop_item_for_target(paths, selector, account, None, dry_run, confirmed)
}

/// Write or upload a Workshop create operation with an explicit runtime target.
///
/// # Errors
///
/// Returns discovery, packaging, provider, or receipt errors.
pub fn create_workshop_item_for_target(
    paths: &EnvironmentPaths,
    selector: &str,
    account: Option<&str>,
    target_triple: Option<&str>,
    dry_run: bool,
    confirmed: bool,
) -> Result<WorkshopOperationReport, String> {
    let targets = target_triple
        .map(|target| vec![target.to_owned()])
        .unwrap_or_default();
    create_workshop_item_for_targets(paths, selector, account, &targets, dry_run, confirmed)
}

/// Write or upload a Workshop create operation with explicit runtime targets.
///
/// # Errors
///
/// Returns discovery, packaging, provider, or receipt errors.
pub fn create_workshop_item_for_targets(
    paths: &EnvironmentPaths,
    selector: &str,
    account: Option<&str>,
    target_triples: &[String],
    dry_run: bool,
    confirmed: bool,
) -> Result<WorkshopOperationReport, String> {
    let catalog = discover(paths)?;
    let artifact = catalog.find(selector).ok_or_else(|| {
        format!("unknown content artifact '{selector}'\nhelp: inspect source content with `content list`")
    })?;
    if artifact.workshop().app_id().is_none() {
        return Err(format!(
            "{} has no [{}.steam].app-id and cannot be created on Workshop",
            artifact.id(),
            artifact.kind()
        ));
    }
    if !dry_run && artifact.workshop().published_file_id().is_some() {
        return Err(format!(
            "{} already has PublishedFileId {}; use `content publish` for updates",
            artifact.id(),
            artifact.workshop().published_file_id().expect("checked")
        ));
    }
    if !dry_run && account.unwrap_or("").trim().is_empty() {
        return Err(
            "real Workshop creation requires --account ACCOUNT after reviewing --dry-run"
                .to_owned(),
        );
    }
    if !dry_run && !confirmed {
        return Err("real Workshop creation requires --yes after reviewing --dry-run".to_owned());
    }

    let package = package_for_targets(paths, artifact.id(), false, target_triples)?;
    let script = write_workshop_script(paths, artifact, package.root(), None, dry_run)?;
    if dry_run {
        let receipt = write_receipt(
            paths.installation(),
            "workshop-create",
            artifact.id(),
            "dry-run",
            Some(&format!("script={}", script.display())),
        )?;
        return Ok(WorkshopOperationReport {
            artifact_id: artifact.id().to_owned(),
            script: Some(script),
            receipt,
            uploaded: false,
            published_file_id: None,
        });
    }

    run_steamcmd_workshop_build(paths, account.expect("account checked"), &script)?;
    let published_file_id = read_workshop_script_published_file_id(&script)?;
    if published_file_id == "0" {
        return Err(format!(
            "SteamCMD did not write a PublishedFileId into '{}'",
            script.display()
        ));
    }
    record_published_file_id(artifact, &published_file_id)?;
    let receipt = write_receipt(
        paths.installation(),
        "workshop-create",
        artifact.id(),
        "created",
        Some(&format!(
            "script={}; published-file-id={}",
            script.display(),
            published_file_id
        )),
    )?;
    Ok(WorkshopOperationReport {
        artifact_id: artifact.id().to_owned(),
        script: Some(script),
        receipt,
        uploaded: true,
        published_file_id: Some(published_file_id),
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
    publish_workshop_item_for_target(
        paths,
        selector,
        account,
        None,
        change_note,
        dry_run,
        confirmed,
    )
}

/// Publish or preview a Workshop item update for one runtime target.
///
/// # Errors
///
/// Returns package, provider, SteamCMD, or authority errors.
pub fn publish_workshop_item_for_target(
    paths: &EnvironmentPaths,
    selector: &str,
    account: Option<&str>,
    target_triple: Option<&str>,
    change_note: Option<&str>,
    dry_run: bool,
    confirmed: bool,
) -> Result<WorkshopOperationReport, String> {
    let targets = target_triple
        .map(|target| vec![target.to_owned()])
        .unwrap_or_default();
    publish_workshop_item_for_targets(
        paths,
        selector,
        account,
        &targets,
        change_note,
        dry_run,
        confirmed,
    )
}

/// Publish or preview a Workshop item update for runtime targets.
///
/// # Errors
///
/// Returns package, provider, SteamCMD, or authority errors.
pub fn publish_workshop_item_for_targets(
    paths: &EnvironmentPaths,
    selector: &str,
    account: Option<&str>,
    target_triples: &[String],
    change_note: Option<&str>,
    dry_run: bool,
    confirmed: bool,
) -> Result<WorkshopOperationReport, String> {
    publish_workshop_items_for_targets(
        paths,
        &[selector.to_owned()],
        account,
        target_triples,
        change_note,
        dry_run,
        confirmed,
    )
    .and_then(|mut reports| {
        reports
            .pop()
            .ok_or_else(|| "no Workshop item was published".to_owned())
    })
}

/// Publish or preview one or more Workshop item updates.
///
/// Multiple real uploads are sent through one SteamCMD process so the authority
/// boundary remains a single interactive provider session.
///
/// # Errors
///
/// Returns discovery, packaging, receipt, or provider errors.
pub fn publish_workshop_items(
    paths: &EnvironmentPaths,
    selectors: &[String],
    account: Option<&str>,
    change_note: Option<&str>,
    dry_run: bool,
    confirmed: bool,
) -> Result<Vec<WorkshopOperationReport>, String> {
    publish_workshop_items_for_target(
        paths,
        selectors,
        account,
        None,
        change_note,
        dry_run,
        confirmed,
    )
}

/// Publish or preview one or more Workshop item updates for one runtime target.
///
/// Multiple real uploads are sent through one SteamCMD process so the authority
/// boundary remains a single interactive provider session.
///
/// # Errors
///
/// Returns discovery, packaging, receipt, or provider errors.
pub fn publish_workshop_items_for_target(
    paths: &EnvironmentPaths,
    selectors: &[String],
    account: Option<&str>,
    target_triple: Option<&str>,
    change_note: Option<&str>,
    dry_run: bool,
    confirmed: bool,
) -> Result<Vec<WorkshopOperationReport>, String> {
    let targets = target_triple
        .map(|target| vec![target.to_owned()])
        .unwrap_or_default();
    publish_workshop_items_for_targets(
        paths,
        selectors,
        account,
        &targets,
        change_note,
        dry_run,
        confirmed,
    )
}

/// Publish or preview one or more Workshop item updates for runtime targets.
///
/// Multiple real uploads are sent through one SteamCMD process so the authority
/// boundary remains a single interactive provider session.
///
/// # Errors
///
/// Returns discovery, packaging, receipt, or provider errors.
pub fn publish_workshop_items_for_targets(
    paths: &EnvironmentPaths,
    selectors: &[String],
    account: Option<&str>,
    target_triples: &[String],
    change_note: Option<&str>,
    dry_run: bool,
    confirmed: bool,
) -> Result<Vec<WorkshopOperationReport>, String> {
    if selectors.is_empty() {
        return Err("at least one Workshop artifact is required".to_owned());
    }
    let catalog = discover(paths)?;
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

    let mut staged = Vec::new();
    for selector in selectors {
        let artifact = catalog.find(selector).ok_or_else(|| {
            format!(
                "unknown content artifact '{selector}'\nhelp: inspect source content with `content list`"
            )
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

        let package = package_for_targets(paths, artifact.id(), false, target_triples)?;
        let script = write_workshop_script(paths, artifact, package.root(), change_note, dry_run)?;
        staged.push((
            artifact.id().to_owned(),
            script,
            artifact
                .workshop()
                .published_file_id()
                .map(ToOwned::to_owned),
        ));
    }

    if !dry_run {
        let scripts = staged
            .iter()
            .map(|(_, script, _)| script.clone())
            .collect::<Vec<_>>();
        run_steamcmd_workshop_builds(paths, account.expect("account checked"), &scripts)?;
    }

    let receipt_status = if dry_run { "dry-run" } else { "uploaded" };
    let mut reports = Vec::new();
    for (artifact_id, script, published_file_id) in staged {
        let receipt = write_receipt(
            paths.installation(),
            "workshop-publish",
            &artifact_id,
            receipt_status,
            Some(&format!("script={}", script.display())),
        )?;
        reports.push(WorkshopOperationReport {
            artifact_id,
            script: Some(script),
            receipt,
            uploaded: !dry_run,
            published_file_id,
        });
    }
    Ok(reports)
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
        published_file_id: None,
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

struct InstallContext<'a> {
    installation: &'a InstallationPaths,
    paths: Option<&'a EnvironmentPaths>,
    catalog: Option<&'a ContentCatalog>,
    layout: &'a ContentLayout,
    account: Option<&'a str>,
    runtime_targets: &'a [RuntimeTarget],
}

fn install_selector(
    context: &InstallContext<'_>,
    selector: &str,
    index: &mut ContentIndex,
    visiting: &mut BTreeSet<String>,
    reports: &mut Vec<InstallReport>,
) -> Result<(), String> {
    if let Some(catalog) = context.catalog
        && let Some(artifact) = catalog.find(selector)
    {
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
            if reports
                .iter()
                .any(|report| report.artifact_id() == dependency.id())
            {
                continue;
            }
            if catalog.by_id(dependency.id()).is_some() {
                install_selector(context, dependency.id(), index, visiting, reports)?;
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
        let package = package_with_runtime_targets(
            context
                .paths
                .expect("source catalog only exists with source paths"),
            artifact.id(),
            false,
            context.runtime_targets.to_vec(),
        )?;
        let report = install_package(
            context.installation,
            context.layout,
            index,
            artifact,
            package.root(),
            package.fingerprint().clone(),
            "source-package",
        )?;
        reports.push(report);
        visiting.remove(artifact.id());
        return Ok(());
    }

    if index.cache_by_selector(selector).is_none()
        && let Some(seed) = root_content_seed(context.installation, selector)?
    {
        let acquired = acquire_workshop_item(
            context.installation,
            seed.app_id,
            &seed.workshop_id,
            context.account,
        )?;
        index.caches.insert(
            acquired.artifact_id().to_owned(),
            CacheRecord {
                artifact_id: acquired.artifact_id().to_owned(),
                workshop_id: Some(seed.workshop_id),
                cache_root: acquired.cache_root().to_path_buf(),
                fingerprint: acquired.fingerprint().clone(),
                acquired_at: now_seconds(),
            },
        );
    }

    let cache = index
        .cache_by_selector(selector)
        .map(|(_, cache)| cache.clone())
        .ok_or_else(|| {
            format!(
                "cannot install '{selector}': no matching source artifact, app-owned cache entry, or root content seed exists\nhelp: run `content acquire ARTIFACT`, open a source, or add a [[root.content]] seed"
            )
        })?;
    let pseudo = read_deployed_manifest(&cache.cache_root)?;
    let pseudo_id = pseudo.id().to_owned();
    if !visiting.insert(pseudo_id.clone()) {
        return Err(format!("content dependency cycle includes {}", pseudo.id()));
    }
    let result = (|| {
        for dependency in pseudo.dependencies().iter().filter(|item| !item.optional()) {
            if !index.installed.contains_key(dependency.id()) {
                let selector = dependency.workshop_id().unwrap_or_else(|| dependency.id());
                install_selector(context, selector, index, visiting, reports)?;
            }
        }
        for conflict in pseudo.conflicts() {
            if index
                .installed
                .get(conflict.id())
                .is_some_and(|record| record.enabled)
            {
                return Err(format!(
                    "{} conflicts with installed content '{}'",
                    pseudo.id(),
                    conflict.id()
                ));
            }
        }
        let report = install_package(
            context.installation,
            context.layout,
            index,
            &pseudo,
            &cache.cache_root,
            cache.fingerprint,
            "cache",
        )?;
        reports.push(report);
        Ok(())
    })();
    visiting.remove(&pseudo_id);
    result
}

fn install_package(
    installation: &InstallationPaths,
    layout: &ContentLayout,
    index: &mut ContentIndex,
    artifact: &ContentArtifact,
    artifact_root: &Path,
    expected_fingerprint: Fingerprint,
    source: &str,
) -> Result<InstallReport, String> {
    let target = layout.installed().join(artifact.id());
    reset_directory(installation.root(), &target)?;
    fs::create_dir_all(&target).map_err(io("create installed content", &target))?;
    copy_tree(artifact_root, &target, artifact_root, &[])?;
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
            "cannot {} {}: expected artifact root is missing at {}",
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
    fs::rename(&from_root, &to_root).map_err(io("move content artifact root", &from_root))?;
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
                "installed artifact root is missing: {}",
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
        ContentKind::Game => content
            .engine
            .as_ref()
            .map(|reference| reference.to_content_reference("engine", false))
            .into_iter()
            .collect(),
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
    #[serde(default)]
    binaries: Vec<String>,
    #[serde(default)]
    libraries: Vec<String>,
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
struct DeployedManifest {
    schema: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    engine: Option<DeployedContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    game: Option<DeployedContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    packagepack: Option<DeployedContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enginepack: Option<DeployedContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gamepack: Option<DeployedContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    modpack: Option<DeployedContent>,
    #[serde(rename = "engine-mod", skip_serializing_if = "Option::is_none")]
    engine_mod: Option<DeployedContent>,
    #[serde(rename = "game-mod", skip_serializing_if = "Option::is_none")]
    game_mod: Option<DeployedContent>,
    #[serde(rename = "extension-mod", skip_serializing_if = "Option::is_none")]
    extension_mod: Option<DeployedContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct DeployedContent {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    binaries: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    libraries: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    runtime: Vec<RuntimePayload>,
    #[serde(
        default,
        rename = "steam",
        skip_serializing_if = "WorkshopPolicy::is_empty"
    )]
    workshop: WorkshopPolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<ContentReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    conflicts: Vec<ContentReference>,
}

impl DeployedManifest {
    fn from_artifact(artifact: &ContentArtifact, runtime: Vec<RuntimePayload>) -> Self {
        let content = DeployedContent {
            id: artifact.id().to_owned(),
            name: artifact.name().to_owned(),
            version: artifact.version().map(ToOwned::to_owned),
            binaries: artifact.binaries().to_vec(),
            libraries: artifact.libraries().to_vec(),
            runtime,
            workshop: artifact.workshop().clone(),
            dependencies: artifact.dependencies().to_vec(),
            conflicts: artifact.conflicts().to_vec(),
        };
        let mut manifest = Self::empty();
        match artifact.kind() {
            ContentKind::Engine => manifest.engine = Some(content),
            ContentKind::Game => manifest.game = Some(content),
            ContentKind::Packagepack => manifest.packagepack = Some(content),
            ContentKind::Enginepack => manifest.enginepack = Some(content),
            ContentKind::Gamepack => manifest.gamepack = Some(content),
            ContentKind::Modpack => manifest.modpack = Some(content),
            ContentKind::EngineMod => manifest.engine_mod = Some(content),
            ContentKind::GameMod => manifest.game_mod = Some(content),
            ContentKind::ExtensionMod => manifest.extension_mod = Some(content),
        }
        manifest
    }

    fn empty() -> Self {
        Self {
            schema: CONTENT_STATE_SCHEMA,
            engine: None,
            game: None,
            packagepack: None,
            enginepack: None,
            gamepack: None,
            modpack: None,
            engine_mod: None,
            game_mod: None,
            extension_mod: None,
        }
    }

    fn into_artifact(self, root: PathBuf, manifest: PathBuf) -> Result<ContentArtifact, String> {
        if self.schema == 0 {
            return Err(format!(
                "deployed content manifest '{}' has invalid schema 0",
                manifest.display()
            ));
        }
        let mut entries = Vec::new();
        push_deployed(&mut entries, ContentKind::Engine, self.engine);
        push_deployed(&mut entries, ContentKind::Game, self.game);
        push_deployed(&mut entries, ContentKind::Packagepack, self.packagepack);
        push_deployed(&mut entries, ContentKind::Enginepack, self.enginepack);
        push_deployed(&mut entries, ContentKind::Gamepack, self.gamepack);
        push_deployed(&mut entries, ContentKind::Modpack, self.modpack);
        push_deployed(&mut entries, ContentKind::EngineMod, self.engine_mod);
        push_deployed(&mut entries, ContentKind::GameMod, self.game_mod);
        push_deployed(&mut entries, ContentKind::ExtensionMod, self.extension_mod);
        match entries.len() {
            1 => {
                let (kind, content) = entries.pop().expect("length checked");
                Ok(ContentArtifact {
                    id: content.id,
                    name: content.name,
                    kind,
                    root,
                    manifest,
                    version: content.version,
                    binaries: content.binaries,
                    libraries: content.libraries,
                    runtime: content.runtime,
                    dependencies: content.dependencies,
                    conflicts: content.conflicts,
                    workshop: content.workshop,
                })
            }
            0 => Err("deployed content manifest has no content section".to_owned()),
            _ => Err("deployed content manifest has multiple content sections".to_owned()),
        }
    }
}

fn push_deployed(
    entries: &mut Vec<(ContentKind, DeployedContent)>,
    kind: ContentKind,
    content: Option<DeployedContent>,
) {
    if let Some(content) = content {
        entries.push((kind, content));
    }
}

fn stage_deployed_artifact(
    paths: &EnvironmentPaths,
    package_root: &Path,
    artifact: &ContentArtifact,
    runtime_targets: &[RuntimeTarget],
) -> Result<(), String> {
    validate_runtime_output_names("binaries", artifact.binaries())?;
    validate_runtime_output_names("libraries", artifact.libraries())?;
    copy_tree(
        artifact.root(),
        package_root,
        artifact.root(),
        default_exclusions(),
    )?;
    let mut runtime = artifact.runtime().to_vec();
    for runtime_target in runtime_targets {
        let staged_runtime = stage_runtime_outputs(paths, package_root, artifact, runtime_target)?;
        if staged_runtime.is_empty() {
            continue;
        }
        runtime.retain(|payload| payload.target != runtime_target.triple());
        runtime.push(RuntimePayload {
            target: runtime_target.triple().to_owned(),
            binaries: staged_runtime.binaries,
            libraries: staged_runtime.libraries,
        });
    }
    runtime.sort_by(|left, right| left.target.cmp(&right.target));
    write_deployed_manifest(package_root, artifact, runtime)
}

fn stage_runtime_outputs(
    paths: &EnvironmentPaths,
    package_root: &Path,
    artifact: &ContentArtifact,
    runtime_target: &RuntimeTarget,
) -> Result<StagedRuntimeOutputs, String> {
    let mut staged = StagedRuntimeOutputs::default();
    if artifact.binaries().is_empty() && artifact.libraries().is_empty() {
        return Ok(staged);
    }
    let build_root = content_build_output_root(paths, runtime_target)?;
    let bin_root = package_root.join("bin").join(runtime_target.triple());
    let lib_root = package_root.join("lib").join(runtime_target.triple());
    for binary in artifact.binaries() {
        let source = find_built_binary(&build_root, binary, runtime_target.triple())?;
        staged
            .binaries
            .push(copy_runtime_output(&source, &bin_root)?);
    }
    for library in artifact.libraries() {
        let source = find_built_library(&build_root, library, runtime_target.triple())?;
        staged
            .libraries
            .push(copy_runtime_output(&source, &lib_root)?);
    }
    Ok(staged)
}

#[derive(Debug, Clone)]
struct RuntimeTarget {
    triple: String,
    explicit: bool,
}

impl RuntimeTarget {
    fn new(target_triple: Option<&str>) -> Result<Self, String> {
        match target_triple {
            Some(target) => {
                validate_runtime_target(target)?;
                Ok(Self {
                    triple: target.to_owned(),
                    explicit: true,
                })
            }
            None => Ok(Self::host()),
        }
    }

    fn many(target_triples: &[String]) -> Result<Vec<Self>, String> {
        if target_triples.is_empty() {
            return Ok(vec![Self::host()]);
        }
        let mut seen = BTreeSet::new();
        let mut targets = Vec::new();
        for target in target_triples {
            let runtime_target = Self::new(Some(target))?;
            if !seen.insert(runtime_target.triple.clone()) {
                return Err(format!("duplicate runtime target: {target}"));
            }
            targets.push(runtime_target);
        }
        Ok(targets)
    }

    fn host() -> Self {
        Self {
            triple: host_runtime_target(),
            explicit: false,
        }
    }

    fn triple(&self) -> &str {
        &self.triple
    }
}

fn runtime_target_label(runtime_targets: &[RuntimeTarget]) -> String {
    runtime_targets
        .iter()
        .map(|target| target.triple())
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Debug, Default)]
struct StagedRuntimeOutputs {
    binaries: Vec<String>,
    libraries: Vec<String>,
}

impl StagedRuntimeOutputs {
    fn is_empty(&self) -> bool {
        self.binaries.is_empty() && self.libraries.is_empty()
    }
}

pub(crate) fn host_runtime_target() -> String {
    let arch = std::env::consts::ARCH;
    match (arch, std::env::consts::OS, std::env::consts::FAMILY) {
        ("x86_64", "linux", _) => "x86_64-unknown-linux-gnu".to_owned(),
        ("aarch64", "linux", _) => "aarch64-unknown-linux-gnu".to_owned(),
        ("x86_64", "windows", _) => {
            if cfg!(target_env = "msvc") {
                "x86_64-pc-windows-msvc".to_owned()
            } else {
                "x86_64-pc-windows-gnu".to_owned()
            }
        }
        ("aarch64", "windows", _) => "aarch64-pc-windows-msvc".to_owned(),
        ("x86_64", "macos", _) => "x86_64-apple-darwin".to_owned(),
        ("aarch64", "macos", _) => "aarch64-apple-darwin".to_owned(),
        _ => format!(
            "{arch}-{}-{}",
            std::env::consts::OS,
            std::env::consts::FAMILY
        ),
    }
}

fn validate_runtime_target(target: &str) -> Result<(), String> {
    let valid = !target.is_empty()
        && target
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(format!(
            "runtime target must be a Rust target triple such as x86_64-pc-windows-msvc: {target}"
        ))
    }
}

fn binary_suffix(target: &str) -> &'static str {
    if target.contains("windows") {
        ".exe"
    } else {
        ""
    }
}

fn library_file_names(stem: &str, target: &str) -> Vec<String> {
    if target.contains("windows") {
        vec![format!("{stem}.dll"), format!("{stem}.lib")]
    } else if target.contains("darwin") || target.contains("apple") {
        vec![format!("lib{stem}.dylib"), format!("lib{stem}.rlib")]
    } else {
        vec![format!("lib{stem}.so"), format!("lib{stem}.rlib")]
    }
}

fn content_build_output_root(
    paths: &EnvironmentPaths,
    runtime_target: &RuntimeTarget,
) -> Result<PathBuf, String> {
    #[derive(Deserialize)]
    struct RootManifest {
        workspace: Option<WorkspaceSection>,
    }
    #[derive(Deserialize)]
    struct WorkspaceSection {
        name: String,
    }

    let path = paths.source().root().join(manifest::FILE_NAME);
    let source = fs::read_to_string(&path).map_err(io("read source manifest", &path))?;
    let parsed: RootManifest = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", path.display()))?;
    let name = parsed
        .workspace
        .map(|workspace| workspace.name)
        .ok_or_else(|| {
            format!(
                "content runtime outputs require a [workspace] source manifest: {}",
                path.display()
            )
        })?;
    let root = paths.installation().root().join("output/dev").join(name);
    if runtime_target.explicit {
        Ok(root.join(runtime_target.triple()).join("debug"))
    } else {
        Ok(root.join("debug"))
    }
}

fn find_built_binary(build_root: &Path, name: &str, target: &str) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    let suffix = binary_suffix(target);
    if suffix.is_empty() || name.ends_with(suffix) {
        candidates.push(build_root.join(name));
    } else {
        candidates.push(build_root.join(format!("{name}{suffix}")));
        candidates.push(build_root.join(name));
    }
    find_existing_runtime_output("binary", name, candidates)
}

fn find_built_library(build_root: &Path, name: &str, target: &str) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    if Path::new(name).extension().is_none() {
        for stem in [name.to_owned(), name.replace('-', "_")] {
            for candidate in library_file_names(&stem, target) {
                candidates.push(build_root.join(candidate));
            }
        }
    } else {
        candidates.push(build_root.join(name));
    }
    let mut deduped = Vec::new();
    for candidate in candidates {
        if !deduped.contains(&candidate) {
            deduped.push(candidate);
        }
    }
    find_existing_runtime_output("library", name, deduped)
}

fn find_existing_runtime_output(
    kind: &str,
    name: &str,
    candidates: Vec<PathBuf>,
) -> Result<PathBuf, String> {
    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    let checked = candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "declared content {kind} '{name}' was not built; checked: {checked}\nhelp: run `content build` before `content package` or use `content deploy`"
    ))
}

fn copy_runtime_output(source: &Path, target_directory: &Path) -> Result<String, String> {
    fs::create_dir_all(target_directory)
        .map_err(io("create runtime output directory", target_directory))?;
    let file_name = source
        .file_name()
        .ok_or_else(|| format!("runtime output has no filename: {}", source.display()))?;
    let target = target_directory.join(file_name);
    fs::copy(source, &target).map_err(io("copy runtime output", source))?;
    Ok(file_name.to_string_lossy().into_owned())
}

fn validate_runtime_output_names(label: &str, names: &[String]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for name in names {
        let mut components = Path::new(name).components();
        let valid = matches!(
            components.next(),
            Some(Component::Normal(part)) if part.to_str() == Some(name.as_str())
        ) && components.next().is_none();
        if !valid {
            return Err(format!(
                "content {label} must be file names, not paths: {name}"
            ));
        }
        if !seen.insert(name) {
            return Err(format!("duplicate content {label} entry: {name}"));
        }
    }
    Ok(())
}

fn write_deployed_manifest(
    package_root: &Path,
    artifact: &ContentArtifact,
    runtime: Vec<RuntimePayload>,
) -> Result<(), String> {
    let manifest = DeployedManifest::from_artifact(artifact, runtime);
    let encoded = toml::to_string_pretty(&manifest)
        .map_err(|error| format!("failed to encode deployed content manifest: {error}"))?;
    let path = package_root.join(manifest::FILE_NAME);
    fs::write(&path, encoded).map_err(io("write deployed content manifest", &path))
}

fn read_deployed_manifest(package_root: &Path) -> Result<ContentArtifact, String> {
    let path = package_root.join(manifest::FILE_NAME);
    let source = fs::read_to_string(&path).map_err(io("read deployed content manifest", &path))?;
    let manifest: DeployedManifest = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", path.display()))?;
    manifest.into_artifact(package_root.to_path_buf(), path)
}

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new(prefix: &str, artifact_id: &str) -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let base = std::env::temp_dir();
        for attempt in 0..16 {
            let path = base.join(format!(
                "{}-{}-{}-{}-{}",
                prefix,
                slug(artifact_id),
                std::process::id(),
                stamp,
                attempt
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!(
                        "failed to create temporary directory '{}': {error}",
                        path.display()
                    ));
                }
            }
        }
        Err(format!(
            "failed to create temporary directory for {} after repeated attempts",
            artifact_id
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
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
    artifact_root: &Path,
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
    let mut all_tags = Vec::new();
    for tag in std::iter::once(artifact.kind().to_string()).chain(policy.tags().iter().cloned()) {
        if !all_tags.contains(&tag) {
            all_tags.push(tag);
        }
    }
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
        steam_escape(&artifact_root.display().to_string()),
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

fn run_steamcmd_workshop_build(
    paths: &EnvironmentPaths,
    account: &str,
    script: &Path,
) -> Result<(), String> {
    run_steamcmd_workshop_builds(paths, account, &[script.to_path_buf()])
}

fn run_steamcmd_workshop_builds(
    paths: &EnvironmentPaths,
    account: &str,
    scripts: &[PathBuf],
) -> Result<(), String> {
    if scripts.is_empty() {
        return Err("at least one Workshop build script is required".to_owned());
    }
    let steamcmd = steam::executable(paths)?;
    let mut command = Command::new(&steamcmd);
    command.args(["+login", account]);
    for script in scripts {
        command.arg("+workshop_build_item").arg(script);
    }
    let status = command
        .arg("+quit")
        .current_dir(steamcmd.parent().expect("SteamCMD has a parent"))
        .status()
        .map_err(|error| format!("failed to start SteamCMD: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Steam Workshop build exited with {status}"))
    }
}

fn run_steamcmd_workshop_download(
    installation: &InstallationPaths,
    app_id: u32,
    published_file_id: &str,
    account: &str,
) -> Result<PathBuf, String> {
    let mut roots = run_steamcmd_workshop_downloads(
        installation,
        &[WorkshopDownload {
            app_id,
            published_file_id: published_file_id.to_owned(),
        }],
        account,
    )?;
    roots
        .pop()
        .ok_or_else(|| "SteamCMD returned no Workshop download root".to_owned())
}

#[derive(Debug, Clone)]
struct WorkshopDownload {
    app_id: u32,
    published_file_id: String,
}

fn run_steamcmd_workshop_downloads(
    installation: &InstallationPaths,
    downloads: &[WorkshopDownload],
    account: &str,
) -> Result<Vec<PathBuf>, String> {
    if downloads.is_empty() {
        return Err("at least one Workshop download is required".to_owned());
    }
    let steamcmd = steam::executable_for_installation(installation)?;
    let mut command = Command::new(&steamcmd);
    command.args(["+login", account]);
    for download in downloads {
        command
            .arg("+workshop_download_item")
            .arg(download.app_id.to_string())
            .arg(&download.published_file_id);
    }
    let status = command
        .arg("+quit")
        .current_dir(steamcmd.parent().expect("SteamCMD has a parent"))
        .status()
        .map_err(|error| format!("failed to start SteamCMD: {error}"))?;
    if !status.success() {
        return Err(format!("Steam Workshop download exited with {status}"));
    }
    downloads
        .iter()
        .map(|download| {
            resolve_steamcmd_workshop_download_root(
                installation,
                &steamcmd,
                download.app_id,
                &download.published_file_id,
            )
        })
        .collect()
}

fn resolve_steamcmd_workshop_download_root(
    installation: &InstallationPaths,
    steamcmd: &Path,
    app_id: u32,
    published_file_id: &str,
) -> Result<PathBuf, String> {
    let app_id = app_id.to_string();
    let mut candidates = Vec::new();
    if let Some(parent) = steamcmd.parent() {
        candidates.push(
            parent
                .join("steamapps/workshop/content")
                .join(&app_id)
                .join(published_file_id),
        );
    }
    if let Some(common_dir) = installation.root().parent()
        && common_dir
            .file_name()
            .is_some_and(|name| name.to_string_lossy() == "common")
        && let Some(steamapps_dir) = common_dir.parent()
    {
        candidates.push(
            steamapps_dir
                .join("workshop/content")
                .join(&app_id)
                .join(published_file_id),
        );
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        candidates.push(
            home.join(".local/share/Steam/steamapps/workshop/content")
                .join(&app_id)
                .join(published_file_id),
        );
        candidates.push(
            home.join(".steam/steam/steamapps/workshop/content")
                .join(&app_id)
                .join(published_file_id),
        );
    }
    for download_root in &candidates {
        if download_root.exists() {
            return Ok(download_root.clone());
        }
    }
    let attempted = candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "SteamCMD reported success but no Workshop content folder exists; checked: {attempted}"
    ))
}

fn read_workshop_script_published_file_id(script: &Path) -> Result<String, String> {
    let source = fs::read_to_string(script).map_err(io("read Workshop script", script))?;
    vdf_value(&source, "publishedfileid")
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "Workshop script has no publishedfileid: {}",
                script.display()
            )
        })
}

fn record_published_file_id(
    artifact: &ContentArtifact,
    published_file_id: &str,
) -> Result<(), String> {
    let source = fs::read_to_string(artifact.manifest())
        .map_err(io("read content manifest", artifact.manifest()))?;
    let updated = set_published_file_id(&source, artifact.kind(), published_file_id)?;
    toml::from_str::<AuthoredManifest>(&updated).map_err(|error| {
        format!(
            "refusing to write invalid content manifest '{}': {error}",
            artifact.manifest().display()
        )
    })?;
    fs::write(artifact.manifest(), updated)
        .map_err(io("write content manifest", artifact.manifest()))
}

fn set_published_file_id(
    source: &str,
    kind: ContentKind,
    published_file_id: &str,
) -> Result<String, String> {
    let section = format!("[{}.steam]", kind);
    let mut lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let Some(start) = lines.iter().position(|line| line.trim() == section) else {
        return Err(format!("content manifest has no {section} section"));
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| line.trim_start().starts_with('[').then_some(index))
        .unwrap_or(lines.len());
    let field = format!("published-file-id = \"{}\"", published_file_id);
    if let Some(index) = lines[start + 1..end].iter().position(|line| {
        line.trim_start()
            .strip_prefix("published-file-id")
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    }) {
        lines[start + 1 + index] = field;
    } else {
        let insert_at = lines[start + 1..end]
            .iter()
            .position(|line| {
                line.trim_start()
                    .strip_prefix("app-id")
                    .is_some_and(|rest| rest.trim_start().starts_with('='))
            })
            .map_or(start + 1, |index| start + 2 + index);
        lines.insert(insert_at, field);
    }
    let mut updated = lines.join("\n");
    if source.ends_with('\n') {
        updated.push('\n');
    }
    Ok(updated)
}

fn vdf_value(source: &str, key: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let parts = line.trim().split('"').collect::<Vec<_>>();
        (parts.len() >= 4 && parts[1] == key).then(|| parts[3].to_owned())
    })
}

fn is_published_file_id(selector: &str) -> bool {
    !selector.is_empty() && selector.bytes().all(|byte| byte.is_ascii_digit())
}

fn resolve_download_app_id(
    installation: &InstallationPaths,
    paths: Option<&EnvironmentPaths>,
    selector: &str,
) -> Result<u32, String> {
    if let Some(paths) = paths {
        let catalog = discover(paths)?;
        if let Some(artifact) = catalog.find(selector)
            && let Some(app_id) = artifact.workshop().app_id()
        {
            return Ok(app_id);
        }
    }
    root_steam_app_id(installation)
}

fn root_steam_app_id(installation: &InstallationPaths) -> Result<u32, String> {
    #[derive(Deserialize)]
    struct RootManifest {
        root: Option<RootSection>,
    }
    #[derive(Deserialize)]
    struct RootSection {
        steam: Option<RootSteam>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "kebab-case")]
    struct RootSteam {
        app_id: Option<u32>,
    }

    let path = installation.root().join(manifest::FILE_NAME);
    let source = fs::read_to_string(&path).map_err(io("read root manifest", &path))?;
    let manifest: RootManifest = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", path.display()))?;
    manifest
        .root
        .and_then(|root| root.steam)
        .and_then(|steam| steam.app_id)
        .filter(|app_id| *app_id != 0)
        .ok_or_else(|| {
            format!(
                "cannot infer Steam AppID for Workshop download; '{}' needs [root.steam].app-id",
                path.display()
            )
        })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RootContentSeed {
    id: String,
    app_id: u32,
    workshop_id: String,
    default_launch: Option<String>,
}

fn root_content_seed(
    installation: &InstallationPaths,
    selector: &str,
) -> Result<Option<RootContentSeed>, String> {
    #[derive(Deserialize)]
    struct RootManifest {
        root: Option<RootSection>,
    }
    #[derive(Deserialize)]
    struct RootSection {
        #[serde(default)]
        content: Vec<RootContentSeed>,
    }

    let path = installation.root().join(manifest::FILE_NAME);
    let source = fs::read_to_string(&path).map_err(io("read root manifest", &path))?;
    let manifest: RootManifest = toml::from_str(&source)
        .map_err(|error| format!("failed to parse '{}': {error}", path.display()))?;
    Ok(manifest
        .root
        .into_iter()
        .flat_map(|root| root.content)
        .find(|seed| root_content_seed_matches(seed, selector)))
}

fn root_content_seed_matches(seed: &RootContentSeed, selector: &str) -> bool {
    seed.id == selector
        || seed
            .id
            .rsplit('/')
            .next()
            .is_some_and(|name| name == selector)
        || seed.workshop_id == selector
        || seed
            .default_launch
            .as_deref()
            .is_some_and(|launch| launch == selector)
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
