//! Declarative, command-specific preflight requirements.

use crate::app_local_tools::AppToolRequirement;

/// Requirements checked before one command performs side effects.
#[derive(Debug, Clone)]
pub struct ValidationPlan<'a> {
    pub(super) action: &'a str,
    pub(super) app_local_tools: Vec<AppToolRequirement>,
    pub(super) workspace: bool,
    pub(super) distribution: bool,
}

impl<'a> ValidationPlan<'a> {
    /// Start a targeted validation plan for the named user action.
    pub fn new(action: &'a str) -> Self {
        Self {
            action,
            app_local_tools: Vec::new(),
            workspace: false,
            distribution: false,
        }
    }

    /// Require the selected app-local tool groups.
    #[must_use]
    pub fn app_local_tools(mut self, requirements: &[AppToolRequirement]) -> Self {
        self.app_local_tools.extend_from_slice(requirements);
        self
    }

    /// Require valid root workspace policy.
    #[must_use]
    pub fn workspace(mut self) -> Self {
        self.workspace = true;
        self
    }

    /// Require valid Steam distribution policy.
    #[must_use]
    pub fn distribution(mut self) -> Self {
        self.distribution = true;
        self
    }
}
