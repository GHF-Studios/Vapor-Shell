//! Declarative, command-specific preflight requirements.

use crate::toolchain::Requirement;

/// Requirements checked before one command performs side effects.
#[derive(Debug, Clone)]
pub struct ValidationPlan<'a> {
    pub(super) action: &'a str,
    pub(super) registered_location: bool,
    pub(super) tools: Vec<Requirement>,
    pub(super) workspace: bool,
    pub(super) distribution: bool,
}

impl<'a> ValidationPlan<'a> {
    /// Start a targeted validation plan for the named user action.
    pub fn new(action: &'a str) -> Self {
        Self {
            action,
            registered_location: false,
            tools: Vec::new(),
            workspace: false,
            distribution: false,
        }
    }

    /// Require the executable-derived VAPOR_HOME to match its accepted path.
    #[must_use]
    pub fn registered_location(mut self) -> Self {
        self.registered_location = true;
        self
    }

    /// Require the selected app-local tool groups.
    #[must_use]
    pub fn tools(mut self, tools: &[Requirement]) -> Self {
        self.tools.extend_from_slice(tools);
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
