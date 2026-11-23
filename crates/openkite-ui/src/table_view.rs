//! Table surface: toolbar, sortable header, virtualised body, bulk bar and footer.
//!
//! Markup mirrors the console prototype's `resource-table` markup and class names, so the
//! reference stylesheet applies without overrides. Interaction rules live in
//! [`crate::table`]; this module only wires them to markup.

use dioxus::prelude::*;

use crate::access::{DeniedPanel, Permissions};

use crate::nav::icon_for;
use crate::table::{
    effect_for, health_dots, key_opens_row, pill_class, virtual_window, Row, RowEffect, RowPart,
    Sort, TableState,
};

/// Rows above this count render through a virtual window instead of all at once.
const VIRTUAL_THRESHOLD: usize = 200;
/// Viewport height assumed when the parent owns the real scroll geometry.
const VIEWPORT_PX: f64 = 640.0;

#[derive(Props, Clone, PartialEq, Debug, Default)]
pub struct ResourceTableProps {
    /// Catalogue kind, used to look up the column set and the row icon.
    pub kind: String,
    /// Plural label shown in the filter placeholder and the row-count line.
    pub label: String,
    pub rows: Vec<Row>,
    pub state: TableState,
    /// Namespaces with row counts, for the chip row. Empty hides the row, as the
    /// prototype does for kinds that are not namespaced in practice.
    pub namespaces: Vec<(String, usize)>,
    /// Emitted when a row body is clicked or activated from the keyboard.
    pub on_open: EventHandler<String>,
    /// Emitted with the complete next state after any interaction.
    pub on_state: EventHandler<TableState>,
    /// Current body scroll offset in pixels; owned by the parent so this stays hook-free.
    pub scroll_top: f64,
    /// Emitted when the body scrolls.
    pub on_scroll: EventHandler<f64>,
    /// What the identity may do to this resource; a denied list replaces the table
    /// with the reason, and a denied delete removes the bulk action.
    #[props(default)]
    pub permissions: Permissions,
}

/// Direction glyph for a column, empty when it is not the active sort column.
pub fn sort_mark(sort: &Sort, column: &str) -> &'static str {
    if sort.column == column {
        sort.dir.mark()
    } else {
        ""
    }
}

/// Body cell for a column: the prototype renders kind-specific cells rather than raw text.
fn cell_body(row: &Row, column: &str) -> Element {
    let value = row.cell(column);
    match column {
        "ns" => rsx! {
            span { class: "cell-value",
                span { class: "namespace",
                    button {
                        r#type: "button",
                        class: "ns-link",
                        "data-ns-filter": "{value}",
                        "data-part": "ns-link",
                        title: "Filter everything by namespace {value}",
                        onclick: move |event| {
                            event.stop_propagation();
                        },
                        "{value}"
                    }
                }
            }
        },
        "health" => rsx! {
            span { class: "cell-value",
                span { class: "health-dots",
                    for ok in health_dots(row.cell("ready")) {
                        span { class: if ok { "dot ok" } else { "dot warn" } }
                    }
                }
            }
        },
        "restarts" => rsx! {
            span { class: "cell-value", span { class: "restarts", "{value}" } }
        },
        "ctrl" | "controller" => rsx! {
            span { class: "cell-value", span { class: "controller", "{value}" } }
        },
        "node" => rsx! {
            span { class: "cell-value", span { class: "node", "{value}" } }
        },
        "qos" | "age" => rsx! {
            span { class: "cell-value", span { class: "qos", "{value}" } }
        },
        "status" => rsx! {
            span { class: "cell-value",
                span { class: "pill {pill_class(value)}",
                    span { class: "dot {pill_class(value)}" }
                    "{value}"
                }
            }
        },
        _ => rsx! {
            span { class: "cell-value", "{value}" }
        },
    }
}

#[allow(non_snake_case)]
pub fn ResourceTable(props: ResourceTableProps) -> Element {
    let state = props.state.clone();
    let visible: Vec<Row> = state.visible(&props.rows).into_iter().cloned().collect();
    let total = props.rows.len();
    let cols = state.visible_columns(&props.kind);
    let height = state.density.row_height();
    let icon = icon_for(&props.kind);
    let on_scroll = props.on_scroll;

    // Off-screen rows are never mounted: only the scroll window exists in the DOM.
    let rendered: Vec<Row> = if visible.len() > VIRTUAL_THRESHOLD {
        let window = virtual_window(visible.len(), props.scroll_top, VIEWPORT_PX, height);
        visible[window].to_vec()
    } else {
        visible.clone()
    };

    let selected_count = state.selected.len();
    let selectable: Vec<Row> = visible.clone();
    let all_rows: Vec<Row> = props.rows.clone();
    let emit = props.on_state;
    let open = props.on_open;
    let all_checked = {
        let refs: Vec<&Row> = visible.iter().collect();
        state.all_selected(&refs)
    };
    let lower_label = props.label.to_lowercase();

    if !props.permissions.can_list() {
        let reason = props.permissions.list_denial().map(str::to_owned);
        return rsx! {
            DeniedPanel { resource: props.label.clone(), reason }
        };
    }
    let all_expanded = state.all_expanded(&all_rows);
    let (tree_state, tree_icon, tree_label) = if all_expanded {
        ("expanded", "#i-collapse-all", "Collapse all")
    } else {
        ("collapsed", "#i-expand-all", "Expand all")
    };

    rsx! {
        div { class: "chip-row", id: "nsChips", role: "group", "aria-label": "Namespace filter",
            hidden: props.namespaces.is_empty(),
            button {
                class: if state.namespace.is_none() { "chip active" } else { "chip" },
                r#type: "button",
                "data-part": "ns-all",
                "aria-pressed": if state.namespace.is_none() { "true" } else { "false" },
                onclick: {
                    let st = state.clone();
                    move |_| {
                        let mut next = st.clone();
                        next.namespace = None;
                        next.page = 0;
                        emit.call(next);
                    }
                },
                "All"
                span { class: "chip-count", "{total}" }
            }
            for (ns, count) in props.namespaces.iter().cloned() {
                button {
                    class: if state.namespace.as_deref() == Some(ns.as_str()) { "chip active" } else { "chip" },
                    r#type: "button",
                    "data-part": "ns-chip",
                    "aria-pressed": if state.namespace.as_deref() == Some(ns.as_str()) { "true" } else { "false" },
                    onclick: {
                        let ns = ns.clone();
                        let st = state.clone();
                        move |_| {
                            let mut next = st.clone();
                            next.namespace = Some(ns.clone());
                            next.page = 0;
                            emit.call(next);
                        }
                    },
                    "{ns}"
                    span { class: "chip-count", "{count}" }
                }
            }
        }

        if selected_count > 0 {
            div { class: "bulk-bar", "data-od-id": "bulk-bar",
                span { id: "bulkCount", "{selected_count} selected" }
                button {
                    class: "icon-btn x-clear",
                    r#type: "button",
                    "aria-label": "Clear selection",
                    title: "Clear selection",
                    onclick: {
                        let st = state.clone();
                        move |_| {
                            let mut next = st.clone();
                            next.clear_selection();
                            emit.call(next);
                        }
                    },
                    svg { class: "icon", use { href: "#i-close" } }
                }
                if props.permissions.can_delete() {
                    button { class: "btn btn-danger", r#type: "button",
                        svg { class: "icon", use { href: "#i-trash" } }
                        "Delete"
                    }
                }
            }
        }

        div { class: "panel", "data-od-id": "pod-table-panel",
            div { class: "panel-tools", "data-od-id": "list-filter-bar",
                label { class: "search-field list-search",
                    svg { class: "icon", use { href: "#i-search" } }
                    input {
                        r#type: "search",
                        placeholder: "Filter {lower_label}…",
                        id: "podSearch",
                        "aria-label": "Filter list",
                        autocomplete: "off",
                        value: "{state.query}",
                        oninput: {
                            let st = state.clone();
                            move |event| {
                                let mut next = st.clone();
                                next.query = event.value();
                                next.page = 0;
                                emit.call(next);
                            }
                        },
                    }
                    span { class: "fl-badge", id: "flBadge", "aria-live": "polite",
                        span { class: "all-badge", "All " span { class: "cnt", "{state.visible(&props.rows).len()}" } }
                    }
                    button {
                        r#type: "button",
                        class: "fl-x",
                        id: "flClear",
                        "aria-label": "Clear filter",
                        hidden: state.query.is_empty(),
                        onclick: {
                            let st = state.clone();
                            move |_| {
                                let mut next = st.clone();
                                next.query.clear();
                                next.page = 0;
                                emit.call(next);
                            }
                        },
                        svg { class: "icon", use { href: "#i-close" } }
                    }
                }
                button {
                    class: "icon-btn col-btn",
                    id: "colBtn",
                    r#type: "button",
                    "aria-haspopup": "menu",
                    "aria-expanded": "false",
                    "aria-label": "Choose columns",
                    title: "Columns",
                    "data-od-id": "column-picker",
                    onclick: {
                        let st = state.clone();
                        move |_| {
                            let mut next = st.clone();
                            let target = cols
                                .iter()
                                .rev()
                                .map(|col| col.id)
                                .find(|id| *id != "name" && !st.hidden.contains(*id))
                                .or_else(|| cols.iter().rev().map(|col| col.id).find(|id| *id != "name"));
                            if let Some(target) = target {
                                next.toggle_hidden(target);
                                emit.call(next);
                            }
                        }
                    },
                    svg { class: "icon", "aria-hidden": "true", use { href: "#i-treegrid" } }
                }
                div { class: "tree-bar", id: "treeBar", role: "group", "aria-label": "Expand or collapse all rows",
                    button {
                        class: "icon-btn",
                        id: "expandCollapseAll",
                        r#type: "button",
                        "aria-label": "{tree_label}",
                        title: "{tree_label}",
                        "data-state": "{tree_state}",
                        onclick: {
                            let st = state.clone();
                            let rows = all_rows.clone();
                            move |_| {
                                let mut next = st.clone();
                                next.toggle_expand_all(&rows);
                                emit.call(next);
                            }
                        },
                        svg { class: "icon", use { href: "{tree_icon}" } }
                    }
                }
            }

            div { class: "table-wrap",
                table { class: "resource-table", id: "wlTable",
                    thead {
                        tr { id: "wlHead",
                            th { class: "col-check",
                                input {
                                    r#type: "checkbox",
                                    id: "selAll",
                                    "aria-label": "Select all",
                                    "data-part": "select-all",
                                    checked: all_checked,
                                    onchange: {
                                        let st = state.clone();
                                        let rows = selectable.clone();
                                        move |_| {
                                            let refs: Vec<&Row> = rows.iter().collect();
                                            let mut next = st.clone();
                                            if st.all_selected(&refs) {
                                                next.clear_selection();
                                            } else {
                                                next.select_all(&refs);
                                            }
                                            emit.call(next);
                                        }
                                    },
                                }
                            }
                            for col in cols.iter() {
                                th { "aria-sort": if state.sort.column == col.id { state.sort.dir.aria() } else { "none" },
                                    button {
                                        class: "th-sort",
                                        r#type: "button",
                                        "data-sort": "{col.id}",
                                        onclick: {
                                            let id = col.id;
                                            let st = state.clone();
                                            move |_| {
                                                let mut next = st.clone();
                                                next.sort = st.sort.clicked(id);
                                                emit.call(next);
                                            }
                                        },
                                        "{col.label}"
                                        span { class: "sort-mark", "{sort_mark(&state.sort, col.id)}" }
                                    }
                                }
                            }
                            th { class: "col-act", span { class: "sr-only", "Actions" } }
                        }
                    }
                    tbody {
                        id: "podRows",
                        onscroll: move |event| {
                            on_scroll.call(event.data().scroll_top());
                        },
                        for row in rendered.iter().cloned() {
                            RowView {
                                cols: cols.iter().skip(1).map(|col| (col.id, col.label)).collect::<Vec<_>>(),
                                first_label: cols.first().map(|col| col.label).unwrap_or("Name"),
                                icon: icon.to_string(),
                                row: row.clone(),
                                state: state.clone(),
                                on_open: open,
                                on_state: emit,
                            }
                        }
                    }
                }
            }

            if visible.is_empty() {
                div { class: "empty-state", id: "wlEmpty", "No resources match the current filters." }
            }

            div { class: "panel-footer",
                span { id: "rowCount",
                    "Showing " span { class: "cnt", "{visible.len()}" } " of "
                    span { class: "cnt", "{total}" } " {lower_label}"
                }
                span { class: "footer-hint", "Select a row for details · ⋮ for actions" }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq, Debug)]
pub struct RowViewProps {
    pub row: Row,
    pub first_label: &'static str,
    pub cols: Vec<(&'static str, &'static str)>,
    pub icon: String,
    pub state: TableState,
    pub on_open: EventHandler<String>,
    pub on_state: EventHandler<TableState>,
}

#[allow(non_snake_case)]
fn RowView(props: RowViewProps) -> Element {
    let row = props.row.clone();
    let state = props.state.clone();
    let selected = state.selected.contains(&row.id);
    let expanded = state.expanded.contains(&row.id);
    let has_kids = !row.kids.is_empty();
    let on_open = props.on_open;
    let emit = props.on_state;

    let mut class = String::new();
    if expanded {
        class.push_str("parent-open ");
    }
    if selected {
        class.push_str("checked ");
    }

    let row_for_click = row.clone();
    let row_for_key = row.clone();
    let row_for_check = row.clone();
    let state_for_check = state.clone();
    let row_for_tree = row.clone();
    let state_for_tree = state.clone();
    let drill = row.cells.get("drill").cloned().unwrap_or_default();
    let orb = row.cells.get("orb").cloned().unwrap_or_default();
    let icon = format!("#i-{}", props.icon);
    let name = row.name.clone();

    rsx! {
        tr {
            tabindex: "0",
            "data-id": "{row.id}",
            "data-open": "inspector",
            class: "{class}",
            onclick: move |_| {
                if let RowEffect::OpenInspector = effect_for(RowPart::Body) {
                    on_open.call(row_for_click.id.clone());
                }
            },
            onkeydown: move |event: KeyboardEvent| {
                let opens = match event.key() {
                    Key::Enter => true,
                    Key::Character(ref text) => key_opens_row(text),
                    _ => false,
                };
                if opens {
                    event.prevent_default();
                    if let RowEffect::OpenInspector = effect_for(RowPart::Body) {
                        on_open.call(row_for_key.id.clone());
                    }
                }
            },
            td { class: "col-check", "data-label": "Select",
                input {
                    r#type: "checkbox",
                    class: "row-check",
                    "data-part": "checkbox",
                    "aria-label": "Select {name}",
                    checked: selected,
                    onclick: move |event| {
                        event.stop_propagation();
                        if let RowEffect::ToggleSelection = effect_for(RowPart::Checkbox) {
                            let mut next = state_for_check.clone();
                            next.toggle_selected(&row_for_check.id);
                            emit.call(next);
                        }
                    },
                }
            }
            td { "data-label": "{props.first_label}", "data-first": "",
                span { class: "cell-value row-first",
                    if has_kids {
                        button {
                            r#type: "button",
                            class: "tree-toggle",
                            "data-ptoggle": "{row.id}",
                            "data-part": "tree-toggle",
                            "aria-expanded": if expanded { "true" } else { "false" },
                            "aria-label": if expanded { "Collapse {name}" } else { "Expand {name}" },
                            onclick: move |event| {
                                event.stop_propagation();
                                if let RowEffect::Ignore = effect_for(RowPart::TreeToggle) {
                                    let mut next = state_for_tree.clone();
                                    next.toggle_expanded(&row_for_tree.id);
                                    emit.call(next);
                                }
                            },
                            svg { class: "icon", use { href: "#i-chevron" } }
                        }
                    }
                    span { class: "resource-name",
                        svg { class: "icon", use { href: "{icon}" } }
                        "{row.name}"
                    }
                    if !drill.is_empty() {
                        a {
                            class: "drill",
                            href: "{drill}",
                            "data-part": "drill",
                            "aria-label": "Open {name}",
                            onclick: move |event| {
                                event.stop_propagation();
                            },
                            svg { class: "icon", use { href: "#i-arrow-up-right" } }
                        }
                    }
                }
            }
            for col in props.cols.iter() {
                td {
                    "data-label": "{col.1}",
                    {cell_body(&row, col.0)}
                }
            }
            td { class: "col-act", "data-label": "Actions",
                if !orb.is_empty() {
                    button {
                        r#type: "button",
                        class: "orb-count",
                        "data-part": "orb",
                        "aria-label": "{orb} related resources",
                        onclick: move |event| {
                            event.stop_propagation();
                        },
                        "{orb}"
                    }
                }
                button {
                    r#type: "button",
                    class: "row-act",
                    "data-part": "action",
                    "aria-label": "Actions for {name}",
                    "aria-haspopup": "menu",
                    onclick: move |event| {
                        event.stop_propagation();
                    },
                    svg { class: "icon", use { href: "#i-kebab" } }
                }
            }
        }
        if expanded {
            for kid in row.kids.iter().cloned() {
                tr { class: "child", tabindex: "0", "data-key": "{kid.id}",
                    td { class: "col-check" }
                    td { "data-label": "{props.first_label}", "data-first": "",
                        span { class: "cell-value row-first",
                            span { class: "resource-name",
                                svg { class: "icon", use { href: "{icon}" } }
                                "{kid.name}"
                            }
                        }
                    }
                    for col in props.cols.iter() {
                        td { "data-label": "{col.1}", {cell_body(&kid, col.0)} }
                    }
                    td { class: "col-act" }
                }
            }
        }
    }
}
