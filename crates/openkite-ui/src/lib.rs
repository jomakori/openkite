//! OpenKite console UI: shell, navigation and the page surfaces rendered inside it.

pub mod access;
pub mod actions_view;
pub mod nav;
pub mod shell;

pub use access::{DeniedPanel, Permissions, Verb as AccessVerb};
pub use nav::{
    build_nav, coverage_gaps, entry_for_path, CatalogueKind, NavEntry, NavGroup, CATALOGUE, VIEWS,
};
pub use shell::{AppShell, AppShellProps, Connection, Counts};

pub mod inspector;
pub mod inspector_view;
pub mod logs;
pub mod logs_view;
pub mod manifest_edit;
pub mod manifest_edit_view;
pub mod metrics;
pub mod overview;
pub mod overview_view;
pub mod palette;
pub mod palette_view;
pub mod settings;
pub mod settings_view;
pub mod table;
pub mod table_columns;
pub mod table_view;

pub use inspector::{
    action_bar, actions_for, default_schema, has_tab, tab_ids, tabs_for, Container, Detail, Event,
    Gauge, InspectorData, InspectorState, Manifest, QuickAction, ResourceLink, SchemaNode, Tab,
};
pub use inspector_view::{Inspector, InspectorProps};
pub use logs::{DockAction, Level, LogDock, LogLine, LogPane, PodChoice};
pub use logs_view::{Dock, DockProps};
pub use manifest_edit::{
    diff, highlight, lint, Conflict, Diagnostic, FieldChange, Lint, ManifestEdit, Phase, Severity,
    FIELD_MANAGER,
};
pub use manifest_edit_view::{ManifestEditor, ManifestEditorProps};
pub use overview::{
    BoardAction, CapacityGauge, Interest, Lane, NodeRow, OverviewData, OverviewLayout, ProblemRow,
    Size, WidgetId,
};
pub use overview_view::{Overview, OverviewProps};
pub use palette::{Command, Entry, Hit, Palette, COMMANDS, RESULT_LIMIT};
pub use palette_view::{PaletteOverlay, PaletteOverlayProps};
// `Density` already names the table row density, so the settings one stays qualified.
pub use settings::{
    builtin_themes, parse_zed_theme, Appearance, Settings, Theme, ThemeError, SHORTCUTS, TOKENS,
};
pub use settings_view::{SettingsSheet, SettingsSheetProps, ThemeScope, ThemeScopeProps};
pub use table::{columns, Column, Density, Row, RowEffect, RowPart, Sort, SortDir, TableState};
pub use table_view::{ResourceTable, ResourceTableProps};
