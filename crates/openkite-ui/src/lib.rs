//! OpenKite console UI: shell, navigation and the page surfaces rendered inside it.

pub mod nav;
pub mod shell;

pub use nav::{
    build_nav, coverage_gaps, entry_for_path, CatalogueKind, NavEntry, NavGroup, CATALOGUE, VIEWS,
};
pub use shell::{AppShell, AppShellProps, Connection, Counts};
