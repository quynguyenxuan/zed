//! Provides hooks for customizing the behavior of the command palette.

#![deny(missing_docs)]

use std::{any::TypeId, rc::Rc};

use collections::HashSet;
use derive_more::{Deref, DerefMut};
use gpui::{Action, App, BorrowAppContext, Global, Task, WeakEntity};
use workspace::Workspace;

/// Initializes the command palette hooks.
pub fn init(cx: &mut App) {
    cx.set_global(GlobalCommandPaletteFilter::default());
    cx.set_global(GlobalDynamicCommandRegistry::default());
}

/// A filter for the command palette.
#[derive(Default)]
pub struct CommandPaletteFilter {
    hidden_namespaces: HashSet<&'static str>,
    hidden_action_types: HashSet<TypeId>,
    /// Actions that have explicitly been shown. These should be shown even if
    /// they are in a hidden namespace.
    shown_action_types: HashSet<TypeId>,
}

#[derive(Deref, DerefMut, Default)]
struct GlobalCommandPaletteFilter(CommandPaletteFilter);

impl Global for GlobalCommandPaletteFilter {}

impl CommandPaletteFilter {
    /// Returns the global [`CommandPaletteFilter`], if one is set.
    pub fn try_global(cx: &App) -> Option<&CommandPaletteFilter> {
        cx.try_global::<GlobalCommandPaletteFilter>()
            .map(|filter| &filter.0)
    }

    /// Returns a mutable reference to the global [`CommandPaletteFilter`].
    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<GlobalCommandPaletteFilter>()
    }

    /// Updates the global [`CommandPaletteFilter`] using the given closure.
    pub fn update_global<F>(cx: &mut App, update: F)
    where
        F: FnOnce(&mut Self, &mut App),
    {
        if cx.has_global::<GlobalCommandPaletteFilter>() {
            cx.update_global(|this: &mut GlobalCommandPaletteFilter, cx| update(&mut this.0, cx))
        }
    }

    /// Returns whether the given [`Action`] is hidden by the filter.
    pub fn is_hidden(&self, action: &dyn Action) -> bool {
        let name = action.name();
        let namespace = name.split("::").next().unwrap_or("malformed action name");

        // If this action has specifically been shown then it should be visible.
        if self.shown_action_types.contains(&action.type_id()) {
            return false;
        }

        self.hidden_namespaces.contains(namespace)
            || self.hidden_action_types.contains(&action.type_id())
    }

    /// Hides all actions in the given namespace.
    pub fn hide_namespace(&mut self, namespace: &'static str) {
        self.hidden_namespaces.insert(namespace);
    }

    /// Shows all actions in the given namespace.
    pub fn show_namespace(&mut self, namespace: &'static str) {
        self.hidden_namespaces.remove(namespace);
    }

    /// Hides all actions with the given types.
    pub fn hide_action_types<'a>(&mut self, action_types: impl IntoIterator<Item = &'a TypeId>) {
        for action_type in action_types {
            self.hidden_action_types.insert(*action_type);
            self.shown_action_types.remove(action_type);
        }
    }

    /// Shows all actions with the given types.
    pub fn show_action_types<'a>(&mut self, action_types: impl IntoIterator<Item = &'a TypeId>) {
        for action_type in action_types {
            self.shown_action_types.insert(*action_type);
            self.hidden_action_types.remove(action_type);
        }
    }
}

/// The result of intercepting a command palette command.
#[derive(Debug)]
pub struct CommandInterceptItem {
    /// The action produced as a result of the interception.
    pub action: Box<dyn Action>,
    /// The display string to show in the command palette for this result.
    pub string: String,
    /// The character positions in the string that match the query.
    /// Used for highlighting matched characters in the command palette UI.
    pub positions: Vec<usize>,
}

/// The result of intercepting a command palette command.
#[derive(Default, Debug)]
pub struct CommandInterceptResult {
    /// The items
    pub results: Vec<CommandInterceptItem>,
    /// Whether or not to continue to show the normal matches
    pub exclusive: bool,
}

/// An interceptor for the command palette.
#[derive(Clone)]
pub struct GlobalCommandPaletteInterceptor(
    Rc<dyn Fn(&str, WeakEntity<Workspace>, &mut App) -> Task<CommandInterceptResult>>,
);

impl Global for GlobalCommandPaletteInterceptor {}

impl GlobalCommandPaletteInterceptor {
    /// Sets the global interceptor.
    ///
    /// This will override the previous interceptor, if it exists.
    pub fn set(
        cx: &mut App,
        interceptor: impl Fn(&str, WeakEntity<Workspace>, &mut App) -> Task<CommandInterceptResult>
        + 'static,
    ) {
        cx.set_global(Self(Rc::new(interceptor)));
    }

    /// Clears the global interceptor.
    pub fn clear(cx: &mut App) {
        if cx.has_global::<Self>() {
            cx.remove_global::<Self>();
        }
    }

    /// Intercepts the given query from the command palette.
    pub fn intercept(
        query: &str,
        workspace: WeakEntity<Workspace>,
        cx: &mut App,
    ) -> Option<Task<CommandInterceptResult>> {
        let interceptor = cx.try_global::<Self>()?;
        let handler = interceptor.0.clone();
        Some(handler(query, workspace, cx))
    }
}

/// A command dynamically registered at runtime by an extension.
pub struct DynamicCommand {
    /// Display name shown in the command palette (e.g. `"gui_test: open panel"`).
    pub name: String,
    /// Extension that registered this command, used to bulk-unregister on extension unload.
    pub extension_id: std::sync::Arc<str>,
    /// Opaque identifier passed back to the extension via `run-extension-command`.
    pub command_id: std::sync::Arc<str>,
}

/// Registry of commands dynamically registered by extensions at runtime.
#[derive(Default)]
pub struct DynamicCommandRegistry {
    commands: Vec<DynamicCommand>,
}

impl DynamicCommandRegistry {
    /// Registers a new dynamic command.
    pub fn register(&mut self, command: DynamicCommand) {
        self.commands.push(command);
    }

    /// Removes all commands registered by the given extension.
    pub fn unregister_extension(&mut self, extension_id: &str) {
        self.commands
            .retain(|c| c.extension_id.as_ref() != extension_id);
    }

    /// Iterates over all registered dynamic commands.
    pub fn commands(&self) -> impl Iterator<Item = &DynamicCommand> {
        self.commands.iter()
    }
}

/// Global wrapper for [`DynamicCommandRegistry`], stored in the GPUI app context.
#[derive(Default)]
pub struct GlobalDynamicCommandRegistry(pub DynamicCommandRegistry);

impl Global for GlobalDynamicCommandRegistry {}
