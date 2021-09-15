#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

pub mod channel;
pub mod config;

use self::{channel::*, config::*};
use std::path::PathBuf;

pub enum ConfigFile {
    Link(PathBuf),
    File(ConfigFileFile),
}

pub struct ConfigFileFile {
    pub path: PathBuf,
    pub contents: Cfg,
}

pub struct Cfg {}

macro_rules! opt_vec {
    ($($item:expr),*$(,)?) => {{
        let mut vec = Vec::with_capacity(opt_vec!(@count $($item:expr;)*));
        $(if let Some(item) = $item {
            vec.push(item);
        })*
        vec.shrink_to_fit();
        vec
    }};
    (@count $head:expr; $($item:expr;)*) => {
        1 + opt_vec!(@count $($item;)*)
    };
    (@count) => {
        0
    };
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
    let panel_ids = 0..;
    let plugin_ids = 1..;
    let channel = Channel::new(
        "xfce4-panel",
        "1.0",
        opt_vec![
            Some(Property::new("configver", Value::int(2))),
            get_opt!(&config.panels).map(|panels| {
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
                                    panel_props(panel, plugin_ids.clone()),
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
                                        plugin_props(plugin_id, plugin);
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
        ],
    );
    (channel, config_files)
}

fn panel_props(
    panel: &ConfigPanel,
    plugin_ids: impl Iterator<Item = i32>,
) -> Value<'static> {
    Value::empty(opt_vec![
        get_opt!(&panel.display.general.mode)
            .map(|mode| { Property::new("mode", Value::uint(mode.discrim())) }),
        get_opt!(&panel.display.general.locked).map(|locked| Property::new(
            "position-locked",
            Value::bool(!*locked)
        )),
        get_opt!(&panel.display.general.auto_hide).map(|auto_hide| {
            Property::new("autohide-behavior", Value::uint(auto_hide.discrim()))
        }),
        get_opt!(&panel.display.general.reserve_border_space).map(
            |reserve_border_space| Property::new(
                "disable-struts",
                Value::bool(!*reserve_border_space)
            )
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
    ])
}

fn plugin_type(plugin: &ConfigPanelItem) -> &'static str {
    match plugin {
        ConfigPanelItem::Launcher(_) => "launcher",
        ConfigPanelItem::Separator(_) => "separator",
        ConfigPanelItem::ActionButtons(_) => "actions",
        ConfigPanelItem::ApplicationsMenu(_) => "applicationsmenu",
        ConfigPanelItem::WhiskerMenu(_) => "whiskermenu",
    }
}

fn plugin_props(
    plugin_id: i32,
    plugin: &ConfigPanelItem,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    match plugin {
        ConfigPanelItem::Launcher(launcher) => {
            plugin_launcher_props(plugin_id, launcher)
        },
        ConfigPanelItem::Separator(separator) => {
            plugin_separator_props(plugin_id, separator)
        },
        ConfigPanelItem::ActionButtons(action_buttons) => {
            plugin_action_buttons_props(plugin_id, action_buttons)
        },
        ConfigPanelItem::ApplicationsMenu(applications_menu) => {
            plugin_applications_menu_props(plugin_id, applications_menu)
        },
        ConfigPanelItem::WhiskerMenu(_) => todo!(),
    }
}

fn plugin_launcher_props(
    plugin_id: i32,
    launcher: &ConfigPanelItemLauncher,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    // TODO: better item_ids scheme
    // needs to be globally incrementing
    // also start with time_secs * 10
    let item_ids = 1..;
    (
        opt_vec![
            get_opt!(&launcher.items).map(|items| Property::new(
                "items",
                Value::array(
                    item_ids
                        .clone()
                        .take(items.len())
                        .map(|item_id| {
                            Value::string(format!("{}.desktop", item_id))
                        })
                        .collect(),
                ),
            )),
            get_opt!(&launcher.show_tooltips).map(|show_tooltips| {
                Property::new("disable-tooltips", Value::bool(!show_tooltips))
            }),
            get_opt!(&launcher.label_instead_of_icon).map(
                |label_instead_of_icon| Property::new(
                    "show-label",
                    Value::bool(*label_instead_of_icon),
                )
            ),
            get_opt!(&launcher.show_last_used_item).map(
                |show_last_used_item| {
                    Property::new(
                        "move-first",
                        Value::bool(*show_last_used_item),
                    )
                }
            ),
            get_opt!(&launcher.arrow_position).map(|arrow_position| {
                Property::new(
                    "arrow-position",
                    Value::uint(arrow_position.discrim()),
                )
            }),
        ],
        item_ids
            .zip(launcher.items.iter().flatten())
            .map(|(item_id, item)| match item {
                ConfigPanelItemLauncherItem::Str(s) => {
                    // TODO: support URL items
                    ConfigFile::Link(PathBuf::from(s))
                },
                ConfigPanelItemLauncherItem::Struct(item) => {
                    ConfigFile::File(ConfigFileFile {
                        path: PathBuf::from(format!(
                            "launcher-{}/{}.desktop",
                            plugin_id, item_id
                        )),
                        contents: Cfg {
                            // TODO: launcher cfg file
                        },
                    })
                },
            })
            .collect(),
    )
}

fn plugin_separator_props(
    _plugin_id: i32,
    separator: &ConfigPanelItemSeparator,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    (
        opt_vec![
            get_opt!(&separator.style).map(|style| Property::new(
                "style",
                Value::uint(style.discrim()),
            )),
            get_opt!(&separator.expand)
                .map(|expand| Property::new("expand", Value::bool(*expand))),
        ],
        Vec::new(),
    )
}

fn plugin_action_buttons_props(
    _plugin_id: i32,
    action_buttons: &ConfigPanelItemActionButtons,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    (
        opt_vec![
            get_opt!(&action_buttons.general.appearance).map(|appearance| {
                Property::new("appearance", Value::uint(appearance.discrim()))
            }),
            get_opt!(&action_buttons.general.title).map(|title| Property::new(
                "title",
                Value::uint(title.discrim()),
            )),
            get_opt!(&action_buttons.general.custom_title).map(
                |custom_title| {
                    Property::new(
                        "custom-title",
                        Value::string(custom_title.clone()),
                    )
                }
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
                }
            ),
        ],
        Vec::new(),
    )
}

fn plugin_applications_menu_props(
    _plugin_id: i32,
    applications_menu: &ConfigPanelItemApplicationsMenu,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    (
        opt_vec![
            get_opt!(&applications_menu.appearance.show_generic_names).map(
                |show_generic_names| Property::new(
                    "show-generic-names",
                    Value::bool(*show_generic_names)
                )
            ),
            get_opt!(&applications_menu.appearance.show_menu_icons).map(
                |show_menu_icons| Property::new(
                    "show-menu-icons",
                    Value::bool(*show_menu_icons)
                )
            ),
            get_opt!(&applications_menu.appearance.show_tooltips).map(
                |show_tooltips| Property::new(
                    "show-tooltips",
                    Value::bool(*show_tooltips)
                )
            ),
            get_opt!(&applications_menu.appearance.show_button_title).map(
                |show_button_title| Property::new(
                    "show-button-title",
                    Value::bool(*show_button_title)
                )
            ),
            get_opt!(&applications_menu.appearance.button_title).map(
                |button_title| Property::new(
                    "button-title",
                    Value::string(button_title.clone())
                )
            ),
            get_opt!(&applications_menu.appearance.button_icon).map(
                |button_icon| Property::new(
                    "button-icon",
                    Value::string(button_icon.clone())
                )
            ),
        ],
        Vec::new(),
    )
}
