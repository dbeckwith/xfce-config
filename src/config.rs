config_types_proc_macro::config_types! {
    panels: [{
        display: {
            // TODO: panel position
            general: {
                mode: ("horizontal" | "vertical" | "deskbar"),
                locked: bool,
                auto_hide: ("never" | "auto" | "always"),
                reserve_border_space: bool,
            },
            measurements: {
                row_size: uint,
                row_count: uint,
                length: uint,
                auto_size: bool,
            },
        },
        appearance: {
            // TODO: panel appearance
        },
        items: [(
            | {
                r#type: "launcher",
                items: [(
                    | str
                    | {
                        name: str,
                        comment: str,
                        command: str,
                        icon: str,
                        startup_notification: bool,
                        run_in_terminal: bool,
                    }
                )],
                show_tooltips: bool,
                label_instead_of_icon: bool,
                show_last_used_item: bool,
                arrow_position: (
                    | "default"
                    | "north"
                    | "west"
                    | "east"
                    | "south"
                    | "inside-button"
                ),
            }
            | {
                r#type: "separator",
                style: ("transparent" | "separator" | "handle" | "dots"),
                expand: bool,
            }
            | {
                r#type: "action-buttons",
                general: {
                    appearance: ("action-buttons" | "session-menu"),
                    title: ("full-name" | "username" | "user-id" | "custom"),
                    custom_title: str,
                },
                actions: {
                    items: {
                        // TODO: different schema to allow reordering
                        lock_screen: bool,
                        switch_user: bool,
                        separator1: bool,
                        suspend: bool,
                        hibernate: bool,
                        hybrid_sleep: bool,
                        separator2: bool,
                        shutdown: bool,
                        restart: bool,
                        separator3: bool,
                        logout: bool,
                        logout_dialog: bool,
                    },
                    show_confirmation_dialog: bool,
                },
            }
            | {
                r#type: "whiskermenu",
                appearance: {
                    panel_button: {
                        display: ("icon" | "title" | "icon-and-title"),
                        title: str,
                        icon: str,
                        single_row: bool,
                    },
                    menu: {
                        show_generic_app_names: bool,
                        show_category_names: bool,
                        show_app_descriptions: bool,
                        show_app_tooltips: bool,
                        show_menu_hierarchy: bool,
                        item_icon_size: (
                            | "none"
                            | "very-small"
                            | "smaller"
                            | "small"
                            | "normal"
                            | "large"
                            | "larger"
                            | "very-large"
                        ),
                        category_icon_size: (
                            | "none"
                            | "very-small"
                            | "smaller"
                            | "small"
                            | "normal"
                            | "large"
                            | "larger"
                            | "very-large"
                        ),
                        background_opacity: uint,
                        width: uint,
                        height: uint,
                    },
                },
                behavior: {
                    menu: {
                        switch_categories_on_hover: bool,
                        search_next_to_panel_button: bool,
                        commands_next_to_search: bool,
                        categories_next_to_panel_button: bool,
                    },
                    recently_used: {
                        max_items: uint,
                        ignore_favorites: bool,
                        always_show: bool,
                    },
                },
                commands: {
                    settings: {
                        command: str,
                        show: bool,
                    },
                    lockscreen: {
                        command: str,
                        show: bool,
                    },
                    switchuser: {
                        command: str,
                        show: bool,
                    },
                    logout: {
                        command: str,
                        show: bool,
                    },
                    menueditor: {
                        command: str,
                        show: bool,
                    },
                    profile: {
                        command: str,
                        show: bool,
                    },
                },
                search_actions: [{
                    name: str,
                    pattern: str,
                    command: str,
                    is_regex: bool,
                }],
            }
        )],
    }],
}
