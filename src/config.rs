use serde::de;
use std::fmt;

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
                r#type: "cpu-graph";
                appearance: {
                    color1: color;
                    color2: color;
                    color3: color;
                    background_color: color;
                    mode: (
                        | "disabled"
                        | "normal"
                        | "led"
                        | "no-history"
                        | "grid"
                    );
                    color_mode: ("solid" | "gradient" | "fire");
                    show_current_usage_bar: bool;
                    bars_color: color;
                    show_frame: bool;
                    show_border: bool;
                };
                advanced: {
                    update_interval: (
                        | "fastest"
                        | "fast"
                        | "normal"
                        | "slow"
                        | "slowest"
                    );
                    // TODO: better way to represent tracked_core?
                    /// `0` will track all cores, `n` will track the nth core
                    tracked_core: uint;
                    width: uint;
                    threshold: uint;
                    associated_command: str;
                    run_in_terminal: bool;
                    use_startup_notification: bool;
                    non_linear_time_scale: bool;
                    per_core_history_graphs: bool;
                    // TODO: per-core history graph spacing
                };
            }
            | {
                r#type: "directory-menu";
                appearance: {
                    base_directory: str;
                    icon: str;
                };
                menu: {
                    show_open_folder: bool;
                    show_open_in_terminal: bool;
                    show_new_folder: bool;
                    show_new_text_document: bool;
                };
                filtering: {
                    file_pattern: str;
                    show_hidden_files: bool;
                };
            }
            | {
                r#type: "free-space-checker";
                configuration: {
                    mount_point: str;
                    warning_limit: uint;
                    urgent_limit: uint;
                };
                user_interface: {
                    show_name: bool;
                    name: str;
                    show_size: bool;
                    show_meter: bool;
                    show_button: bool;
                };
            }
            | {
                r#type: "network-monitor";
                show_label: bool;
                label: str;
                network_device: str;
                update_interval_ms: uint;
                show_values_as_bits: bool;
                auto_max: bool;
                max_in_bytes: uint;
                max_out_bytes: uint;
                style: ("bars" | "values" | "bars-and-values");
                bar_color_in: color;
                bar_color_out: color;
                colorize_values: bool;
            }
            // TODO: notification-plugin
            // config is in xfce4-notifyd perchannel-xml
            | {
                r#type: "pulseaudio";
                volume_keyboard_shortcuts: bool;
                volume_notifications: bool;
                audio_mixer: str;
                control_media_players: bool;
                playback_keyboard_shortcuts: bool;
            }
            | {
                r#type: "screenshot";
                capture_region: (
                    | "entire-screen"
                    | "active-window"
                    | "selection-region"
                );
                capture_cursor: bool;
                capture_delay: uint;
            }
            | {
                r#type: "show-desktop";
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

#[derive(Debug)]
pub struct Color(pub u8, pub u8, pub u8);

impl<'de> de::Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Color;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "color")
            }

            fn visit_str<E>(self, mut v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                v = v.strip_prefix('#').unwrap_or(v);
                if v.len() == 6 {
                    let r =
                        u8::from_str_radix(&v[0..2], 16).map_err(E::custom)?;
                    let g =
                        u8::from_str_radix(&v[2..4], 16).map_err(E::custom)?;
                    let b =
                        u8::from_str_radix(&v[4..6], 16).map_err(E::custom)?;
                    Ok(Color(r, g, b))
                } else if v.len() == 3 {
                    let r =
                        u8::from_str_radix(&v[0..1], 16).map_err(E::custom)?;
                    let g =
                        u8::from_str_radix(&v[1..2], 16).map_err(E::custom)?;
                    let b =
                        u8::from_str_radix(&v[2..3], 16).map_err(E::custom)?;
                    let r = r | (r << 4);
                    let g = g | (g << 4);
                    let b = b | (b << 4);
                    Ok(Color(r, g, b))
                } else {
                    Err(E::custom("expected 3- or 6-character hex code"))
                }
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let r = seq.next_element()?.ok_or_else(|| {
                    de::Error::invalid_length(0, &"expected exactly 3 elements")
                })?;
                let g = seq.next_element()?.ok_or_else(|| {
                    de::Error::invalid_length(1, &"expected exactly 3 elements")
                })?;
                let b = seq.next_element()?.ok_or_else(|| {
                    de::Error::invalid_length(2, &"expected exactly 3 elements")
                })?;
                if seq.next_element::<de::IgnoredAny>()?.is_some() {
                    return Err(de::Error::invalid_length(
                        4,
                        &"expected exactly 3 elements",
                    ));
                }
                Ok(Color(r, g, b))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}
