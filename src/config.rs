config_types_proc_macro::config_types! {
    panels: [{
        display: {
            // TODO: panel position
            general: {
                mode: ("horizontal" | "vertical" | "deskbar");
                locked: bool;
                auto_hide: ("never" | "auto" | "always");
                reserve_border_space: bool;
            };
            measurements: {
                row_size: uint;
                row_count: uint;
                length: uint;
                auto_size: bool;
            };
        };
        appearance: {
            // TODO: panel appearance
        };
        items: [(
            | {
                r#type: "launcher";
                items: [(
                    | str
                    | {
                        name: str;
                        comment: str;
                        command: str;
                        icon: str;
                        startup_notification: bool;
                        run_in_terminal: bool;
                    }
                )];
                show_tooltips: bool;
                label_instead_of_icon: bool;
                show_last_used_item: bool;
                arrow_position: (
                    | "default"
                    | "north"
                    | "west"
                    | "east"
                    | "south"
                    | "inside-button"
                );
            }
            | {
                r#type: "separator";
                style: ("transparent" | "separator" | "handle" | "dots");
                expand: bool;
            }
            | {
                r#type: "action-buttons";
                general: {
                    appearance: ("action-buttons" | "session-menu");
                    title: ("full-name" | "username" | "user-id" | "custom");
                    custom_title: str;
                };
                actions: {
                    items: [{
                        r#type: (
                            | "separator"
                            | "lock-screen"
                            | "switch-user"
                            | "suspend"
                            | "hibernate"
                            | "hybrid-sleep"
                            | "shutdown"
                            | "restart"
                            | "logout"
                            | "logout-dialog"
                        );
                        enabled: bool;
                    }];
                    show_confirmation_dialog: bool;
                };
            }
            | {
                r#type: "applications-menu";
                appearance: {
                    show_generic_names: bool;
                    show_menu_icons: bool;
                    show_tooltips: bool;
                    show_button_title: bool;
                    button_title: str;
                    button_icon: str;
                };
                // TODO: support custom menu file
            }
            | {
                r#type: "clock";
                time_settings: {
                    timezone: str;
                };
                appearance: {
                    layout: (
                        | "analog"
                        | "binary"
                        | "digital"
                        | "fuzzy"
                        | "lcd"
                    );
                    tooltip_format: str;
                };
                clock_options: {
                    show_seconds: bool;
                    show_military: bool;
                    flash_time_separators: bool;
                    show_am_pm: bool;
                };
            }
            | {
                r#type: "whisker-menu";
                appearance: {
                    panel_button: {
                        display: ("icon" | "title" | "icon-and-title");
                        title: str;
                        icon: str;
                        single_row: bool;
                    };
                    menu: {
                        show_generic_app_names: bool;
                        show_category_names: bool;
                        show_app_descriptions: bool;
                        show_app_tooltips: bool;
                        show_menu_hierarchy: bool;
                        item_icon_size: (
                            | "none"
                            | "very-small"
                            | "smaller"
                            | "small"
                            | "normal"
                            | "large"
                            | "larger"
                            | "very-large"
                        );
                        category_icon_size: (
                            | "none"
                            | "very-small"
                            | "smaller"
                            | "small"
                            | "normal"
                            | "large"
                            | "larger"
                            | "very-large"
                        );
                        background_opacity: uint;
                        width: uint;
                        height: uint;
                    };
                };
                behavior: {
                    menu: {
                        switch_categories_on_hover: bool;
                        search_next_to_panel_button: bool;
                        commands_next_to_search: bool;
                        categories_next_to_panel_button: bool;
                    };
                    recently_used: {
                        max_items: uint;
                        ignore_favorites: bool;
                        always_show: bool;
                    };
                };
                commands: {
                    settings: {
                        command: str;
                        show: bool;
                    };
                    lockscreen: {
                        command: str;
                        show: bool;
                    };
                    switchuser: {
                        command: str;
                        show: bool;
                    };
                    logout: {
                        command: str;
                        show: bool;
                    };
                    menueditor: {
                        command: str;
                        show: bool;
                    };
                    profile: {
                        command: str;
                        show: bool;
                    };
                };
                search_actions: [{
                    name: str;
                    pattern: str;
                    command: str;
                    is_regex: bool;
                }];
            }
        )];
    }];
}
