//! Anwendungsschale der IDE, nach Verantwortung in Untermodule gegliedert.

mod account;
mod background;
mod build;
mod chrome;
mod commands;
mod debugging;
mod documents;
mod editor_actions;
mod explorer;
mod extensions;
mod feedback;
mod frame;
mod git_panel;
mod painting;
mod panels;
mod project_io;
mod settings_view;
mod tabs;
mod text_sync;

pub(crate) use build::*;
pub(crate) use extensions::ExtensionSection;
pub(crate) use feedback::{FeedbackState, HelpSection};
pub(crate) use git_panel::GitPanelState;
pub(crate) use painting::*;
pub(crate) use project_io::*;
pub(crate) use text_sync::*;
