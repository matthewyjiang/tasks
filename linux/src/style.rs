use gtk4 as gtk;

pub fn install_css(floating_panel_fade_ms: u64, task_editor_inner_padding: i32) {
    let provider = gtk::CssProvider::new();
    let css = r#"
        .fa-icon,
        .fa-icon label,
        button.fa-icon label {
            font-family: "Font Awesome 7 Free", "Font Awesome 6 Free", sans-serif;
            font-weight: 900;
        }
        button,
        row,
        entry,
        textview,
        .sidebar-row,
        .task-row,
        .markdown-preview,
        .task-editor-title,
        .task-editor-body,
        .task-calendar,
        .task-calendar label {
            transition: background __FLOATING_PANEL_FADE_MS__ms ease-out,
                        border-color __FLOATING_PANEL_FADE_MS__ms ease-out,
                        color __FLOATING_PANEL_FADE_MS__ms ease-out,
                        opacity __FLOATING_PANEL_FADE_MS__ms ease-out;
        }
        button:active {
            background: color-mix(in srgb, @window_fg_color 14%, transparent);
            box-shadow: inset 0 1px 3px color-mix(in srgb, black 22%, transparent);
        }
        button.flat:active {
            background: color-mix(in srgb, @window_fg_color 12%, transparent);
            box-shadow: inset 0 1px 3px color-mix(in srgb, black 18%, transparent);
        }
        .tsk-sidebar {
            background: @sidebar_bg_color;
            padding: 10px 10px 0 12px;
        }
        .sidebar-search {
            border-radius: 8px;
            min-height: 30px;
        }
        .sidebar-list {
            background: transparent;
        }
        .sidebar-list row,
        .sidebar-row {
            border-radius: 10px;
            margin: 2px 0;
            color: @window_fg_color;
            background: transparent;
        }
        .sidebar-list row:hover,
        .sidebar-row:hover {
            background: color-mix(in srgb, @window_fg_color 6%, transparent);
        }
        .sidebar-list row:active,
        .sidebar-row:active,
        .tsk-sidebar button:active,
        .tsk-sidebar button.flat:active {
            background: color-mix(in srgb, @window_fg_color 14%, transparent);
            box-shadow: inset 0 1px 3px color-mix(in srgb, black 18%, transparent);
        }
        .sidebar-list row:selected,
        .sidebar-row:selected {
            background: color-mix(in srgb, @window_fg_color 8%, transparent);
            color: @window_fg_color;
        }
        .sidebar-list row:selected:hover,
        .sidebar-row:selected:hover {
            background: color-mix(in srgb, @window_fg_color 10%, transparent);
        }
        .sidebar-list row:selected label {
            color: @window_fg_color;
        }
        .sidebar-static-row {
            color: @window_fg_color;
            padding: 4px 10px;
            border-radius: 6px;
        }
        .sidebar-icon {
            min-width: 16px;
            font-size: 12px;
            font-weight: 800;
        }
        .sidebar-icon-inbox { color: #64d2ff; }
        .sidebar-icon-today { color: #ff9f0a; }
        .sidebar-icon-upcoming { color: #0a84ff; }
        .sidebar-icon-anytime { color: #8e8e93; }
        .sidebar-icon-done { color: #30d158; }
        .sidebar-count {
            color: @dim_label_color;
            font-size: 12px;
            font-weight: 700;
        }
        .sidebar-bottom-bar,
        .content-bottom-bar {
            border-top: 1px solid @borders;
            padding-top: 8px;
        }
        .sidebar-bottom-bar {
            margin-left: -12px;
            margin-right: -10px;
            padding-left: 12px;
            padding-right: 10px;
            padding-bottom: 10px;
        }
        .content-bottom-bar {
            padding: 8px 18px 10px 18px;
        }
        .search-panel {
            padding: 10px;
            border-radius: 16px;
            background: @popover_bg_color;
            color: @popover_fg_color;
            box-shadow: 0 12px 36px color-mix(in srgb, black 24%, transparent);
            transition: opacity __FLOATING_PANEL_FADE_MS__ms ease-out;
        }
        .move-list-panel {
            padding: 20px;
            border-radius: 20px;
            background: color-mix(in srgb, @popover_bg_color 94%, @accent_color 6%);
            color: @popover_fg_color;
            border: 1px solid color-mix(in srgb, @accent_color 14%, @borders);
            box-shadow: 0 18px 48px color-mix(in srgb, black 30%, transparent);
            transition: opacity __FLOATING_PANEL_FADE_MS__ms ease-out;
        }
        .settings-panel {
            padding: 0;
            border-radius: 20px;
            background: color-mix(in srgb, @popover_bg_color 94%, @accent_color 6%);
            color: @popover_fg_color;
            border: 1px solid color-mix(in srgb, @accent_color 14%, @borders);
            box-shadow: 0 18px 48px color-mix(in srgb, black 30%, transparent);
            transition: opacity __FLOATING_PANEL_FADE_MS__ms ease-out;
        }
        .settings-nav {
            background: @sidebar_bg_color;
            padding: 12px 10px;
            border-radius: 20px 0 0 20px;
        }
        .settings-nav row {
            background: transparent;
            border-radius: 10px;
            margin: 2px 0;
            color: @window_fg_color;
        }
        .settings-nav row:hover {
            background: color-mix(in srgb, @window_fg_color 6%, transparent);
        }
        .settings-nav row:selected {
            background: color-mix(in srgb, @window_fg_color 8%, transparent);
        }
        .settings-content {
            padding: 22px;
        }
        .move-list-title {
            font-size: 20px;
            font-weight: 750;
        }
        .move-list-search {
            min-height: 38px;
            border-radius: 10px;
            font-size: 15px;
        }
        .move-list-results {
            background: transparent;
        }
        .move-list-results row {
            border-radius: 10px;
            margin: 2px 0;
            background: transparent;
        }
        .move-list-results row:hover {
            background: color-mix(in srgb, @window_fg_color 5%, transparent);
        }
        .setup-panel {
            background: @window_bg_color;
            padding: 28px;
        }
        .search-panel-entry {
            min-height: 40px;
            border-radius: 12px;
        }
        .search-results {
            background: transparent;
        }
        .task-editor-panel {
            padding: 24px;
            border-radius: 20px;
            background: color-mix(in srgb, @popover_bg_color 94%, @accent_color 6%);
            color: @popover_fg_color;
            border: 1px solid color-mix(in srgb, @accent_color 14%, @borders);
            box-shadow: 0 18px 48px color-mix(in srgb, black 30%, transparent);
            transition: opacity __FLOATING_PANEL_FADE_MS__ms ease-out;
        }
        .task-editor-panel .pane-title {
            font-size: 20px;
            font-weight: 750;
        }
        .task-editor-title {
            font-size: 28px;
            font-weight: 800;
            padding: __TASK_EDITOR_INNER_PADDING__px;
            margin-bottom: 2px;
            border-radius: 8px;
        }
        .task-editor-title:hover,
        .markdown-preview:hover {
            background: color-mix(in srgb, @window_fg_color 5%, transparent);
        }
        .task-editor-body,
        .task-editor-body text {
            font-size: 16px;
            line-height: 1.55;
            background: transparent;
        }
        .task-editor-body {
            border: 1px solid transparent;
            border-radius: 8px;
            padding: __TASK_EDITOR_INNER_PADDING__px;
        }
        .task-editor-body:focus,
        .task-editor-body:focus-within {
            border-color: @accent_color;
        }
        .task-editor-field-label {
            color: @dim_label_color;
            font-size: 14px;
            font-weight: 700;
        }
        .task-editor-field,
        .task-editor-button {
            min-height: 38px;
            font-size: 15px;
        }
        .task-editor-meta {
            margin-top: 2px;
            margin-bottom: 2px;
        }
        .markdown-preview {
            padding: __TASK_EDITOR_INNER_PADDING__px;
            border: 1px solid transparent;
            border-radius: 8px;
            background: transparent;
            font-size: 16px;
            line-height: 1.55;
        }
        .search-result-row {
            border-radius: 10px;
        }
        .pane-title {
            font-size: 22px;
            font-weight: 750;
            letter-spacing: -0.03em;
        }
        .task-list {
            background: transparent;
        }
        .task-row {
            border-radius: 6px;
            margin: 1px 0;
            background: transparent;
        }
        .task-row:hover {
            background: color-mix(in srgb, @window_fg_color 5%, transparent);
        }
        .confirm-button {
            background: @accent_bg_color;
            color: @accent_fg_color;
            border-radius: 999px;
            font-weight: 800;
        }
        .confirm-button:hover {
            background: color-mix(in srgb, @accent_bg_color 78%, @window_fg_color 22%);
            color: @accent_fg_color;
            box-shadow: 0 2px 8px color-mix(in srgb, @accent_bg_color 35%, transparent);
        }
        .confirm-button:active {
            background: color-mix(in srgb, @accent_bg_color 68%, black 32%);
            color: @accent_fg_color;
            box-shadow: inset 0 2px 5px color-mix(in srgb, black 35%, transparent);
        }
        .sync-status {
            color: @dim_label_color;
            font-size: 12px;
            opacity: 0.75;
        }
        .status-dot {
            color: @accent_color;
            font-size: 18px;
            font-weight: 700;
            min-width: 28px;
            min-height: 28px;
            padding: 0;
            border-radius: 999px;
        }
        .status-dot:hover {
            background: color-mix(in srgb, @accent_color 12%, transparent);
        }
        .task-title {
            font-size: 16px;
            font-weight: 650;
        }
        entry.rename-entry,
        entry.rename-entry:focus,
        entry.rename-entry:focus-within {
            background: transparent;
            border: none;
            border-bottom: 2px solid transparent;
            border-radius: 0;
            box-shadow: none;
            outline: none;
            padding-left: 0;
            padding-right: 0;
        }
        entry.rename-entry.renaming,
        entry.rename-entry.renaming:focus,
        entry.rename-entry.renaming:focus-within {
            border-bottom-color: @accent_color;
        }
        .task-summary {
            color: @dim_label_color;
            font-size: 13px;
        }
        .task-row-expanded,
        .task-row-expanded:hover {
            background: color-mix(in srgb, @accent_color 6%, @card_bg_color);
        }
        .task-row-editor {
            padding-top: 8px;
        }
        entry.task-inline-title,
        entry.task-inline-title:focus,
        entry.task-inline-title:focus-within {
            background: transparent;
            border: none;
            box-shadow: none;
            outline: none;
            padding: 0;
            font-size: 16px;
            font-weight: 650;
        }
        textview.task-inline-notes,
        textview.task-inline-notes text {
            background: transparent;
            border: none;
            box-shadow: none;
            color: @window_fg_color;
            padding: 0;
        }
        textview.task-inline-notes text {
            color: color-mix(in srgb, @window_fg_color 86%, transparent);
        }
        .task-inline-notes-placeholder {
            color: @dim_label_color;
            padding: 0;
        }
        .task-inline-actions button {
            min-width: 28px;
            min-height: 28px;
            padding: 3px 8px;
            border-radius: 999px;
        }
        .task-menu-heading {
            color: @dim_label_color;
            font-size: 12px;
            font-weight: 700;
            margin-top: 6px;
        }
        .task-date-popover {
            padding: 10px;
            background: @card_bg_color;
            border-radius: 14px;
        }
        .task-date-quick-button {
            border: none;
            border-radius: 999px;
            box-shadow: none;
            padding: 6px 14px;
            font-weight: 700;
            background: transparent;
            color: @window_fg_color;
        }
        .task-date-quick-button:hover {
            background: color-mix(in srgb, @accent_color 12%, transparent);
        }
        .task-date-quick-icon {
            color: #f6a000;
        }
        .task-calendar,
        .task-calendar * {
            font-family: sans-serif;
        }
        .task-calendar {
            border: none;
            border-radius: 12px;
            box-shadow: none;
            padding: 4px;
            background: transparent;
            color: @window_fg_color;
        }
        .task-calendar button {
            border: none;
            border-radius: 8px;
            box-shadow: none;
            background: transparent;
            color: @window_fg_color;
        }
        .task-calendar button:hover,
        .task-calendar label:hover,
        .task-calendar grid label:hover {
            background: color-mix(in srgb, @accent_color 14%, transparent);
            border-radius: 8px;
        }
        .task-calendar:selected,
        .task-calendar label:selected,
        .task-calendar grid label:selected {
            background: @accent_bg_color;
            color: @accent_fg_color;
            border-radius: 8px;
        }
        .task-calendar-no-selection:selected,
        .task-calendar-no-selection label:selected,
        .task-calendar-no-selection grid label:selected {
            background: transparent;
            color: @window_fg_color;
        }
        .editor-title {
            font-size: 26px;
            font-weight: 800;
            letter-spacing: -0.04em;
            border: none;
            box-shadow: none;
            background: transparent;
            padding: 4px 0;
        }
        entry.task-editor-title,
        entry.task-editor-title text,
        textview.task-editor-body text {
            padding: __TASK_EDITOR_INNER_PADDING__px;
        }
        .notes-card {
            border-radius: 0;
            background: transparent;
            border-top: 1px solid @borders;
            padding: 10px 0;
        }
        .editor-notes {
            font-size: 14px;
            line-height: 1.45;
            background: transparent;
        }
        .empty-title {
            font-size: 22px;
            font-weight: 700;
        }
        "#
    .replace(
        "__FLOATING_PANEL_FADE_MS__",
        &floating_panel_fade_ms.to_string(),
    )
    .replace(
        "__TASK_EDITOR_INNER_PADDING__",
        &task_editor_inner_padding.to_string(),
    );
    provider.load_from_data(&css);

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
