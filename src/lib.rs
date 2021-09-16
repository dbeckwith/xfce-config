#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

pub mod cfg;
pub mod channel;
pub mod config;

use self::{cfg::*, channel::*, config::*};
use std::{array, ops::RangeFrom, path::PathBuf, time::SystemTime};

pub enum ConfigFile {
    Link(ConfigFileLink),
    File(ConfigFileFile),
}

pub struct ConfigFileLink {
    pub path: PathBuf,
    pub link_from: PathBuf,
}

pub struct ConfigFileFile {
    pub path: PathBuf,
    pub contents: cfg::Cfg,
}

trait OptVec<T> {
    fn opt_vec(self) -> Vec<T>;
}

impl<T, const N: usize> OptVec<T> for [Option<T>; N] {
    fn opt_vec(self) -> Vec<T> {
        array::IntoIter::new(self).flatten().collect()
    }
}

macro_rules! get_opt {
    (& $first:ident . $second:ident $(. $rest:ident)*) => {
        get_opt!(
            @build
            $first.$second.as_ref();
            $second;
            $($rest)*
        )
    };
    (@build $expr:expr; $prev:ident; $head:ident $($rest:ident)*) => {
        get_opt!(
            @build
            $expr.and_then(|$prev| $prev.$head.as_ref());
            $head;
            $($rest)*
        )
    };
    (@build $expr:expr; $prev:ident;) => {
        $expr
    };
}

pub fn convert(config: Config) -> (Channel<'static>, Vec<ConfigFile>) {
    let mut config_files = Vec::new();
    let panel_ids =
        RangeFrom { start: 0 }.take(config.panels.as_ref().map_or(0, Vec::len));
    let plugin_ids = RangeFrom { start: 1 };
    let mut launcher_item_ids = RangeFrom {
        start: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            * 10,
    };
    let channel = Channel::new(
        "xfce4-panel",
        "1.0",
        [
            Some(Property::new("configver", Value::int(2))),
            get_opt!(&config.panels).map(|panels| {
                let mut plugin_ids = plugin_ids.clone();
                Property::new(
                    "panels",
                    Value::new(
                        TypedValue::Array(
                            panel_ids.clone().map(Value::int).collect(),
                        ),
                        panel_ids
                            .zip(panels.iter())
                            .map(|(panel_id, panel)| {
                                Property::new(
                                    format!("panel-{}", panel_id),
                                    panel_props(panel, &mut plugin_ids),
                                )
                            })
                            .collect(),
                    ),
                )
            }),
            get_opt!(&config.panels).and_then(|panels| {
                let plugins = plugin_ids
                    .zip(
                        panels
                            .iter()
                            .flat_map(|panel| panel.items.iter().flatten()),
                    )
                    .map(|(plugin_id, plugin)| {
                        Property::new(
                            format!("plugin-{}", plugin_id),
                            Value::new(
                                TypedValue::String(plugin_type(plugin).into()),
                                {
                                    let (props, plugin_config_files) =
                                        plugin_props(
                                            plugin_id,
                                            plugin,
                                            &mut launcher_item_ids,
                                        );
                                    config_files.extend(plugin_config_files);
                                    props
                                },
                            ),
                        )
                    })
                    .collect::<Vec<_>>();
                (!plugins.is_empty())
                    .then(|| Property::new("plugins", Value::empty(plugins)))
            }),
        ]
        .opt_vec(),
    );
    (channel, config_files)
}

fn panel_props(
    panel: &ConfigPanel,
    plugin_ids: impl Iterator<Item = i32>,
) -> Value<'static> {
    Value::empty(
        [
            get_opt!(&panel.display.general.mode)
                .map(|mode| Property::new("mode", Value::uint(mode.discrim()))),
            get_opt!(&panel.display.general.locked).map(|locked| {
                Property::new("position-locked", Value::bool(!*locked))
            }),
            get_opt!(&panel.display.general.auto_hide).map(|auto_hide| {
                Property::new(
                    "autohide-behavior",
                    Value::uint(auto_hide.discrim()),
                )
            }),
            get_opt!(&panel.display.general.reserve_border_space).map(
                |reserve_border_space| {
                    Property::new(
                        "disable-struts",
                        Value::bool(!*reserve_border_space),
                    )
                },
            ),
            get_opt!(&panel.display.measurements.row_size)
                .map(|row_size| Property::new("size", Value::uint(*row_size))),
            get_opt!(&panel.display.measurements.row_count).map(|row_count| {
                Property::new("nrows", Value::uint(*row_count))
            }),
            get_opt!(&panel.display.measurements.length)
                .map(|length| Property::new("length", Value::uint(*length))),
            get_opt!(&panel.display.measurements.auto_size).map(|auto_size| {
                Property::new("length-adjust", Value::bool(*auto_size))
            }),
            get_opt!(&panel.items).map(|items| {
                Property::new(
                    "plugin_ids",
                    Value::array(
                        plugin_ids.take(items.len()).map(Value::int).collect(),
                    ),
                )
            }),
        ]
        .opt_vec(),
    )
}

fn plugin_type(plugin: &ConfigPanelItem) -> &'static str {
    use ConfigPanelItem::*;
    match plugin {
        Launcher(_) => "launcher",
        Separator(_) => "separator",
        ActionButtons(_) => "actions",
        ApplicationsMenu(_) => "applicationsmenu",
        Clock(_) => "clock",
        CpuGraph(_) => "cpugraph",
        DirectoryMenu(_) => "directorymenu",
        FreeSpaceChecker(_) => "fsguard",
        NetworkMonitor(_) => "netload",
        Pulseaudio(_) => "pulseaudio",
        WhiskerMenu(_) => "whiskermenu",
    }
}

fn plugin_props(
    plugin_id: i32,
    plugin: &ConfigPanelItem,
    launcher_item_ids: impl Iterator<Item = u64>,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    use ConfigPanelItem::*;
    match plugin {
        Launcher(launcher) => {
            plugin_launcher_props(plugin_id, launcher, launcher_item_ids)
        },
        Separator(separator) => plugin_separator_props(plugin_id, separator),
        ActionButtons(action_buttons) => {
            plugin_action_buttons_props(plugin_id, action_buttons)
        },
        ApplicationsMenu(applications_menu) => {
            plugin_applications_menu_props(plugin_id, applications_menu)
        },
        Clock(clock) => plugin_clock_props(plugin_id, clock),
        CpuGraph(cpu_graph) => plugin_cpu_graph_props(plugin_id, cpu_graph),
        DirectoryMenu(directory_menu) => {
            plugin_directory_menu_props(plugin_id, directory_menu)
        },
        FreeSpaceChecker(free_space_checker) => {
            plugin_free_space_checker_props(plugin_id, free_space_checker)
        },
        NetworkMonitor(network_monitor) => {
            plugin_network_monitor_props(plugin_id, network_monitor)
        },
        Pulseaudio(pulseaudio) => {
            plugin_pulseaudio_props(plugin_id, pulseaudio)
        },
        WhiskerMenu(_) => todo!(),
    }
}

fn plugin_launcher_props(
    plugin_id: i32,
    launcher: &ConfigPanelItemLauncher,
    item_ids: impl Iterator<Item = u64>,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    let item_ids = item_ids
        .take(launcher.items.iter().flatten().count())
        .collect::<Vec<_>>();
    (
        [
            get_opt!(&launcher.items).map(|_items| {
                Property::new(
                    "items",
                    Value::array(
                        item_ids
                            .iter()
                            .map(|item_id| {
                                Value::string(format!("{}.desktop", item_id))
                            })
                            .collect(),
                    ),
                )
            }),
            get_opt!(&launcher.show_tooltips).map(|show_tooltips| {
                Property::new("disable-tooltips", Value::bool(!show_tooltips))
            }),
            get_opt!(&launcher.label_instead_of_icon).map(
                |label_instead_of_icon| {
                    Property::new(
                        "show-label",
                        Value::bool(*label_instead_of_icon),
                    )
                },
            ),
            get_opt!(&launcher.show_last_used_item).map(
                |show_last_used_item| {
                    Property::new(
                        "move-first",
                        Value::bool(*show_last_used_item),
                    )
                },
            ),
            get_opt!(&launcher.arrow_position).map(|arrow_position| {
                Property::new(
                    "arrow-position",
                    Value::uint(arrow_position.discrim()),
                )
            }),
        ]
        .opt_vec(),
        item_ids
            .into_iter()
            .zip(launcher.items.iter().flatten())
            .map(|(item_id, item)| {
                plugin_launcher_item(plugin_id, item_id, item)
            })
            .collect(),
    )
}

fn plugin_launcher_item(
    plugin_id: i32,
    item_id: u64,
    item: &ConfigPanelItemLauncherItem,
) -> ConfigFile {
    let path =
        PathBuf::from(format!("launcher-{}/{}.desktop", plugin_id, item_id));
    match item {
        ConfigPanelItemLauncherItem::Str(s) => {
            // TODO: support URL items
            ConfigFile::Link(ConfigFileLink {
                path,
                link_from: PathBuf::from(s),
            })
        },
        ConfigPanelItemLauncherItem::Struct(item) => {
            fn fmt_bool(b: bool) -> String {
                if b { "true" } else { "false" }.to_owned()
            }
            ConfigFile::File(ConfigFileFile {
                path,
                contents: Cfg {
                    root_props: Vec::new(),
                    sections: vec![(
                        "Desktop Entry".to_owned(),
                        [
                            Some(("Version".to_owned(), "1.0".to_owned())),
                            Some(("Type".to_owned(), "Application".to_owned())),
                            get_opt!(&item.name)
                                .map(|name| ("Name".to_owned(), name.clone())),
                            get_opt!(&item.comment).map(|comment| {
                                ("Comment".to_owned(), comment.clone())
                            }),
                            get_opt!(&item.command).map(|command| {
                                ("Exec".to_owned(), command.clone())
                            }),
                            get_opt!(&item.icon)
                                .map(|icon| ("Icon".to_owned(), icon.clone())),
                            get_opt!(&item.startup_notification).map(
                                |startup_notification| {
                                    (
                                        "StartupNotify".to_owned(),
                                        fmt_bool(*startup_notification),
                                    )
                                },
                            ),
                            get_opt!(&item.run_in_terminal).map(
                                |run_in_terminal| {
                                    (
                                        "Terminal".to_owned(),
                                        fmt_bool(*run_in_terminal),
                                    )
                                },
                            ),
                        ]
                        .opt_vec(),
                    )],
                },
            })
        },
    }
}

fn plugin_separator_props(
    _plugin_id: i32,
    separator: &ConfigPanelItemSeparator,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    (
        [
            get_opt!(&separator.style).map(|style| {
                Property::new("style", Value::uint(style.discrim()))
            }),
            get_opt!(&separator.expand)
                .map(|expand| Property::new("expand", Value::bool(*expand))),
        ]
        .opt_vec(),
        Vec::new(),
    )
}

fn plugin_action_buttons_props(
    _plugin_id: i32,
    action_buttons: &ConfigPanelItemActionButtons,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    (
        [
            get_opt!(&action_buttons.general.appearance).map(|appearance| {
                Property::new("appearance", Value::uint(appearance.discrim()))
            }),
            get_opt!(&action_buttons.general.title).map(|title| {
                Property::new("title", Value::uint(title.discrim()))
            }),
            get_opt!(&action_buttons.general.custom_title).map(
                |custom_title| {
                    Property::new(
                        "custom-title",
                        Value::string(custom_title.clone()),
                    )
                },
            ),
            get_opt!(&action_buttons.actions.items).and_then(|items| {
                (!items.is_empty()).then(|| {
                    Property::new(
                        "items",
                        Value::array(
                            items
                                .iter()
                                .filter_map(|item| {
                                    let enabled = item.enabled;
                                    let r#type = item.r#type.as_ref()?;
                                    Some(Value::string(format!(
                                        "{}{}",
                                        if enabled.unwrap_or(true) {
                                            "+"
                                        } else {
                                            "-"
                                        },
                                        r#type.name()
                                    )))
                                })
                                .collect(),
                        ),
                    )
                })
            }),
            get_opt!(&action_buttons.actions.show_confirmation_dialog).map(
                |show_confirmation_dialog| {
                    Property::new(
                        "ask-confirmation",
                        Value::bool(*show_confirmation_dialog),
                    )
                },
            ),
        ]
        .opt_vec(),
        Vec::new(),
    )
}

fn plugin_applications_menu_props(
    _plugin_id: i32,
    applications_menu: &ConfigPanelItemApplicationsMenu,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    (
        [
            get_opt!(&applications_menu.appearance.show_generic_names).map(
                |show_generic_names| {
                    Property::new(
                        "show-generic-names",
                        Value::bool(*show_generic_names),
                    )
                },
            ),
            get_opt!(&applications_menu.appearance.show_menu_icons).map(
                |show_menu_icons| {
                    Property::new(
                        "show-menu-icons",
                        Value::bool(*show_menu_icons),
                    )
                },
            ),
            get_opt!(&applications_menu.appearance.show_tooltips).map(
                |show_tooltips| {
                    Property::new("show-tooltips", Value::bool(*show_tooltips))
                },
            ),
            get_opt!(&applications_menu.appearance.show_button_title).map(
                |show_button_title| {
                    Property::new(
                        "show-button-title",
                        Value::bool(*show_button_title),
                    )
                },
            ),
            get_opt!(&applications_menu.appearance.button_title).map(
                |button_title| {
                    Property::new(
                        "button-title",
                        Value::string(button_title.clone()),
                    )
                },
            ),
            get_opt!(&applications_menu.appearance.button_icon).map(
                |button_icon| {
                    Property::new(
                        "button-icon",
                        Value::string(button_icon.clone()),
                    )
                },
            ),
        ]
        .opt_vec(),
        Vec::new(),
    )
}

fn plugin_clock_props(
    _plugin_id: i32,
    clock: &ConfigPanelItemClock,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    (
        [
            get_opt!(&clock.time_settings.timezone).map(|timezone| {
                Property::new("timezone", Value::string(timezone.clone()))
            }),
            get_opt!(&clock.appearance.layout).map(|layout| {
                Property::new("mode", Value::uint(layout.discrim()))
            }),
            get_opt!(&clock.appearance.tooltip_format).map(|tooltip_format| {
                Property::new(
                    "tooltip-format",
                    Value::string(tooltip_format.clone()),
                )
            }),
            get_opt!(&clock.clock_options.show_seconds).map(|show_seconds| {
                Property::new("show-seconds", Value::bool(*show_seconds))
            }),
            get_opt!(&clock.clock_options.show_military).map(|show_military| {
                Property::new("show-military", Value::bool(*show_military))
            }),
            get_opt!(&clock.clock_options.flash_time_separators).map(
                |flash_time_separators| {
                    Property::new(
                        "flash-separators",
                        Value::bool(*flash_time_separators),
                    )
                },
            ),
            get_opt!(&clock.clock_options.show_am_pm).map(|show_am_pm| {
                Property::new("show-meridiem", Value::bool(*show_am_pm))
            }),
        ]
        .opt_vec(),
        Vec::new(),
    )
}

fn plugin_cpu_graph_props(
    plugin_id: i32,
    cpu_graph: &ConfigPanelItemCpuGraph,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    fn fmt_bool(b: bool) -> String {
        if b { "1" } else { "0" }.to_owned()
    }
    fn fmt_color(Color(r, g, b): &Color) -> String {
        format!("rgb({},{},{})", r, g, b)
    }
    (
        Vec::new(),
        vec![ConfigFile::File(ConfigFileFile {
            path: PathBuf::from(format!("cpugraph-{}.rc", plugin_id)),
            contents: Cfg {
                root_props: [
                    get_opt!(&cpu_graph.appearance.color1).map(|color1| {
                        ("Foreground1".to_owned(), fmt_color(color1))
                    }),
                    get_opt!(&cpu_graph.appearance.color2).map(|color2| {
                        ("Foreground2".to_owned(), fmt_color(color2))
                    }),
                    get_opt!(&cpu_graph.appearance.color3).map(|color3| {
                        ("Foreground3".to_owned(), fmt_color(color3))
                    }),
                    get_opt!(&cpu_graph.appearance.background_color).map(
                        |background_color| {
                            (
                                "Background".to_owned(),
                                fmt_color(background_color),
                            )
                        },
                    ),
                    get_opt!(&cpu_graph.appearance.mode).map(|mode| {
                        ("Mode".to_owned(), mode.discrim().to_string())
                    }),
                    get_opt!(&cpu_graph.appearance.show_current_usage_bar).map(
                        |show_current_usage_bar| {
                            (
                                "Bars".to_owned(),
                                fmt_bool(*show_current_usage_bar),
                            )
                        },
                    ),
                    get_opt!(&cpu_graph.appearance.bars_color).map(
                        |bars_color| {
                            ("BarsColor".to_owned(), fmt_color(bars_color))
                        },
                    ),
                    get_opt!(&cpu_graph.appearance.show_frame).map(
                        |show_frame| {
                            ("Frame".to_owned(), fmt_bool(*show_frame))
                        },
                    ),
                    get_opt!(&cpu_graph.appearance.show_border).map(
                        |show_border| {
                            ("Border".to_owned(), fmt_bool(*show_border))
                        },
                    ),
                    get_opt!(&cpu_graph.advanced.update_interval).map(
                        |update_interval| {
                            (
                                "UpdateInterval".to_owned(),
                                update_interval.discrim().to_string(),
                            )
                        },
                    ),
                    get_opt!(&cpu_graph.advanced.tracked_core).map(
                        |tracked_core| {
                            ("TrackedCore".to_owned(), tracked_core.to_string())
                        },
                    ),
                    get_opt!(&cpu_graph.advanced.width)
                        .map(|width| ("Size".to_owned(), width.to_string())),
                    get_opt!(&cpu_graph.advanced.threshold).map(|threshold| {
                        ("LoadThreshold".to_owned(), threshold.to_string())
                    }),
                    get_opt!(&cpu_graph.advanced.associated_command).map(
                        |associated_command| {
                            (
                                "Command".to_owned(),
                                associated_command.to_string(),
                            )
                        },
                    ),
                    get_opt!(&cpu_graph.advanced.run_in_terminal).map(
                        |run_in_terminal| {
                            (
                                "InTerminal".to_owned(),
                                fmt_bool(*run_in_terminal),
                            )
                        },
                    ),
                    get_opt!(&cpu_graph.advanced.use_startup_notification).map(
                        |use_startup_notification| {
                            (
                                "StartupNotification".to_owned(),
                                fmt_bool(*use_startup_notification),
                            )
                        },
                    ),
                    get_opt!(&cpu_graph.advanced.non_linear_time_scale).map(
                        |non_linear_time_scale| {
                            (
                                "TimeScale".to_owned(),
                                fmt_bool(*non_linear_time_scale),
                            )
                        },
                    ),
                    get_opt!(&cpu_graph.advanced.per_core_history_graphs).map(
                        |per_core_history_graphs| {
                            (
                                "PerCore".to_owned(),
                                fmt_bool(*per_core_history_graphs),
                            )
                        },
                    ),
                ]
                .opt_vec(),
                sections: Vec::new(),
            },
        })],
    )
}

fn plugin_directory_menu_props(
    _plugin_id: i32,
    directory_menu: &ConfigPanelItemDirectoryMenu,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    (
        [
            get_opt!(&directory_menu.appearance.base_directory).map(
                |base_directory| {
                    Property::new(
                        "base-directory",
                        Value::string(base_directory.clone()),
                    )
                },
            ),
            get_opt!(&directory_menu.appearance.icon).map(|icon| {
                Property::new("icon-name", Value::string(icon.clone()))
            }),
            get_opt!(&directory_menu.menu.show_open_folder).map(
                |show_open_folder| {
                    Property::new("open-folder", Value::bool(*show_open_folder))
                },
            ),
            get_opt!(&directory_menu.menu.show_open_in_terminal).map(
                |show_open_in_terminal| {
                    Property::new(
                        "open-in-terminal",
                        Value::bool(*show_open_in_terminal),
                    )
                },
            ),
            get_opt!(&directory_menu.menu.show_new_folder).map(
                |show_new_folder| {
                    Property::new("new-folder", Value::bool(*show_new_folder))
                },
            ),
            get_opt!(&directory_menu.menu.show_new_text_document).map(
                |show_new_text_document| {
                    Property::new(
                        "new-document",
                        Value::bool(*show_new_text_document),
                    )
                },
            ),
            get_opt!(&directory_menu.filtering.file_pattern).map(
                |file_pattern| {
                    Property::new(
                        "file-pattern",
                        Value::string(file_pattern.clone()),
                    )
                },
            ),
            get_opt!(&directory_menu.filtering.show_hidden_files).map(
                |show_hidden_files| {
                    Property::new(
                        "hidden-files",
                        Value::bool(*show_hidden_files),
                    )
                },
            ),
        ]
        .opt_vec(),
        Vec::new(),
    )
}

fn plugin_free_space_checker_props(
    plugin_id: i32,
    free_space_checker: &ConfigPanelItemFreeSpaceChecker,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    fn fmt_bool(b: bool) -> String {
        if b { "true" } else { "false" }.to_owned()
    }
    (
        Vec::new(),
        vec![ConfigFile::File(ConfigFileFile {
            path: PathBuf::from(format!("fsguard-{}.rc", plugin_id)),
            contents: Cfg {
                root_props: [
                    get_opt!(&free_space_checker.configuration.mount_point)
                        .map(|mount_point| {
                            ("mnt".to_owned(), mount_point.clone())
                        }),
                    get_opt!(&free_space_checker.configuration.warning_limit)
                        .map(|warning_limit| {
                            ("yellow".to_owned(), warning_limit.to_string())
                        }),
                    get_opt!(&free_space_checker.configuration.urgent_limit)
                        .map(|urgent_limit| {
                            ("red".to_owned(), urgent_limit.to_string())
                        }),
                    get_opt!(&free_space_checker.user_interface.name)
                        .map(|name| ("label".to_owned(), name.clone())),
                    get_opt!(&free_space_checker.user_interface.show_name).map(
                        |show_name| {
                            ("label_visible".to_owned(), fmt_bool(*show_name))
                        },
                    ),
                    get_opt!(&free_space_checker.user_interface.show_size).map(
                        |show_size| {
                            (
                                "lab_size_visible".to_owned(),
                                fmt_bool(*show_size),
                            )
                        },
                    ),
                    get_opt!(&free_space_checker.user_interface.show_meter)
                        .map(|show_meter| {
                            (
                                "progress_var_visible".to_owned(),
                                fmt_bool(*show_meter),
                            )
                        }),
                    get_opt!(&free_space_checker.user_interface.show_button)
                        .map(|show_button| {
                            ("hide_button".to_owned(), fmt_bool(!show_button))
                        }),
                ]
                .opt_vec(),
                sections: Vec::new(),
            },
        })],
    )
}

fn plugin_network_monitor_props(
    plugin_id: i32,
    network_monitor: &ConfigPanelItemNetworkMonitor,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    fn fmt_bool(b: bool) -> String {
        if b { "true" } else { "false" }.to_owned()
    }
    fn fmt_color(Color(r, g, b): &Color) -> String {
        format!("rgb({},{},{})", r, g, b)
    }
    (
        Vec::new(),
        vec![ConfigFile::File(ConfigFileFile {
            path: PathBuf::from(format!("netload-{}.rc", plugin_id)),
            contents: Cfg {
                root_props: [
                    get_opt!(&network_monitor.show_label).map(|show_label| {
                        ("Use_Label".to_owned(), fmt_bool(*show_label))
                    }),
                    get_opt!(&network_monitor.label)
                        .map(|label| ("Text".to_owned(), label.clone())),
                    get_opt!(&network_monitor.network_device).map(
                        |network_device| {
                            (
                                "Network_Device".to_owned(),
                                network_device.clone(),
                            )
                        },
                    ),
                    get_opt!(&network_monitor.update_interval_ms).map(
                        |update_interval_ms| {
                            (
                                "Update_Interval".to_owned(),
                                update_interval_ms.to_string(),
                            )
                        },
                    ),
                    get_opt!(&network_monitor.show_values_as_bits).map(
                        |show_values_as_bits| {
                            (
                                "Values_As_Bits".to_owned(),
                                fmt_bool(*show_values_as_bits),
                            )
                        },
                    ),
                    get_opt!(&network_monitor.auto_max).map(|auto_max| {
                        ("Auto_Max".to_owned(), fmt_bool(*auto_max))
                    }),
                    get_opt!(&network_monitor.max_in_bytes).map(
                        |max_in_bytes| {
                            ("Max_In".to_owned(), max_in_bytes.to_string())
                        },
                    ),
                    get_opt!(&network_monitor.max_out_bytes).map(
                        |max_out_bytes| {
                            ("Max_Out".to_owned(), max_out_bytes.to_string())
                        },
                    ),
                    get_opt!(&network_monitor.style).map(|style| {
                        use ConfigPanelItemNetworkMonitorStyle::*;
                        (
                            "Show_Bars".to_owned(),
                            fmt_bool(match style {
                                Bars => true,
                                Values => false,
                                BarsAndValues => true,
                            }),
                        )
                    }),
                    get_opt!(&network_monitor.style).map(|style| {
                        use ConfigPanelItemNetworkMonitorStyle::*;
                        (
                            "Show_Values".to_owned(),
                            fmt_bool(match style {
                                Bars => false,
                                Values => true,
                                BarsAndValues => true,
                            }),
                        )
                    }),
                    get_opt!(&network_monitor.bar_color_in).map(
                        |bar_color_in| {
                            ("Color_In".to_owned(), fmt_color(bar_color_in))
                        },
                    ),
                    get_opt!(&network_monitor.bar_color_out).map(
                        |bar_color_out| {
                            ("Color_Out".to_owned(), fmt_color(bar_color_out))
                        },
                    ),
                    get_opt!(&network_monitor.colorize_values).map(
                        |colorize_values| {
                            (
                                "Colorize_Values".to_owned(),
                                fmt_bool(*colorize_values),
                            )
                        },
                    ),
                ]
                .opt_vec(),
                sections: Vec::new(),
            },
        })],
    )
}

fn plugin_pulseaudio_props(
    _plugin_id: i32,
    pulseaudio: &ConfigPanelItemPulseaudio,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    (
        [
            get_opt!(&pulseaudio.volume_keyboard_shortcuts).map(
                |volume_keyboard_shortcuts| {
                    Property::new(
                        "enable-keyboard-shortcuts",
                        Value::bool(*volume_keyboard_shortcuts),
                    )
                },
            ),
            get_opt!(&pulseaudio.volume_notifications).map(
                |volume_notifications| {
                    Property::new(
                        "show-notifications",
                        Value::bool(*volume_notifications),
                    )
                },
            ),
            get_opt!(&pulseaudio.audio_mixer).map(|audio_mixer| {
                Property::new(
                    "mixer-command",
                    Value::string(audio_mixer.clone()),
                )
            }),
            get_opt!(&pulseaudio.control_media_players).map(
                |control_media_players| {
                    Property::new(
                        "enable-mpris",
                        Value::bool(*control_media_players),
                    )
                },
            ),
            get_opt!(&pulseaudio.playback_keyboard_shortcuts).map(
                |playback_keyboard_shortcuts| {
                    Property::new(
                        "enable-multimedia-keys",
                        Value::bool(*playback_keyboard_shortcuts),
                    )
                },
            ),
        ]
        .opt_vec(),
        Vec::new(),
    )
}
