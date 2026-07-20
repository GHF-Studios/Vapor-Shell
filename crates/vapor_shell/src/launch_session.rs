//! App-local launch session state written by platform launch wrappers.
//!
//! The Steam-facing launch path crosses native entrypoints, shell/batch
//! scripts, Vapor Installer, and Vapor Shell. Cross-process status that must be
//! visible after bootstrap belongs in app-local state, not ambient environment
//! variables.

use std::{
    fs,
    path::{Path, PathBuf},
};

const BOOTSTRAP_FAILURE: &str = ".vapor/state/installer/bootstrap-failure.txt";

pub(crate) struct InstallerBootstrapFailure {
    message: String,
    log: Option<PathBuf>,
}

impl InstallerBootstrapFailure {
    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn log(&self) -> Option<&Path> {
        self.log.as_deref()
    }
}

pub(crate) fn installer_bootstrap_failure(app_root: &Path) -> Option<InstallerBootstrapFailure> {
    let source = fs::read_to_string(app_root.join(BOOTSTRAP_FAILURE)).ok()?;
    let mut lines = source.lines();
    let message = lines.next()?.trim();
    if message.is_empty() {
        return None;
    }
    let log = lines
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from);

    Some(InstallerBootstrapFailure {
        message: message.to_owned(),
        log,
    })
}
