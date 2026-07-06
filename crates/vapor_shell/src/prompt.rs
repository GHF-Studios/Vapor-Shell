//! Reedline prompt adapter for the current Vapor context.

use clap_repl::reedline::{Prompt, PromptEditMode, PromptHistorySearch};
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub(crate) struct VaporPrompt {
    context: String,
}

impl VaporPrompt {
    pub(crate) fn new(context: String) -> Self {
        Self { context }
    }
}

impl Prompt for VaporPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.context)
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("> ")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("... ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        _history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        Cow::Borrowed("(reverse-search) ")
    }
}
