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

pub fn convert(config: Config) -> (Channel<'static>, Vec<ConfigFile>) {
    let Config { panels } = config;
    let mut config_files = Vec::new();
    let panel_ids = 0..;
    let plugin_ids = 1..;
    let channel = Channel::new(
        "xfce4-panel",
        "1.0",
        vec![
            Property::new("configver", Value::int(2)),
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
            ),
            Property::new(
                "plugins",
                Value::empty(
                    plugin_ids
                        .zip(panels.into_iter().flat_map(|panel| panel.items))
                        .map(|(plugin_id, plugin)| {
                            Property::new(
                                format!("plugin-{}", plugin_id),
                                Value::new(
                                    TypedValue::String(plugin.r#type().into()),
                                    {
                                        let (props, plugin_config_files) =
                                            plugin_props(plugin_id, plugin);
                                        config_files
                                            .extend(plugin_config_files);
                                        props
                                    },
                                ),
                            )
                        })
                        .collect(),
                ),
            ),
        ],
    );
    (channel, config_files)
}

fn panel_props(
    panel: &ConfigPanel,
    plugin_ids: impl Iterator<Item = i32>,
) -> Value<'static> {
    Value::empty(vec![
        Property::new(
            "mode",
            Value::uint(match panel.display.general.mode {
                ConfigPanelDisplayGeneralMode::Horizontal => 0,
                ConfigPanelDisplayGeneralMode::Vertical => 1,
                ConfigPanelDisplayGeneralMode::Deskbar => 2,
            }),
        ),
        Property::new(
            "autohide-behavior",
            Value::uint(match panel.display.general.auto_hide {
                ConfigPanelDisplayGeneralAutoHide::Never => 0,
                ConfigPanelDisplayGeneralAutoHide::Auto => 1,
                ConfigPanelDisplayGeneralAutoHide::Always => 2,
            }),
        ),
        Property::new("size", Value::uint(panel.display.measurements.row_size)),
        Property::new(
            "nrows",
            Value::uint(panel.display.measurements.row_count),
        ),
        Property::new("length", Value::uint(panel.display.measurements.length)),
        Property::new(
            "length-adjust",
            Value::bool(panel.display.measurements.auto_size),
        ),
        Property::new(
            "plugin_ids",
            Value::array(
                plugin_ids.take(panel.items.len()).map(Value::int).collect(),
            ),
        ),
    ])
}

fn plugin_props(
    plugin_id: i32,
    plugin: ConfigPanelItem,
) -> (Vec<Property<'static>>, Vec<ConfigFile>) {
    match plugin {
        ConfigPanelItem::Launcher(ConfigPanelItemLauncher {
            items,
            show_tooltips,
            label_instead_of_icon,
            show_last_used_item,
            arrow_position,
        }) => {
            // TODO: better item_ids scheme
            // needs to be globally incrementing
            // also start with time_secs * 10
            let item_ids = 1..;
            (
                vec![
                    Property::new(
                        "items",
                        Value::array(
                            item_ids
                                .clone()
                                .map(|item_id| {
                                    Value::string(format!(
                                        "{}.desktop",
                                        item_id
                                    ))
                                })
                                .collect(),
                        ),
                    ),
                    Property::new(
                        "disable-tooltips",
                        Value::bool(!show_tooltips),
                    ),
                    Property::new(
                        "show-label",
                        Value::bool(label_instead_of_icon),
                    ),
                    Property::new(
                        "move-first",
                        Value::bool(show_last_used_item),
                    ),
                    Property::new(
                        "arrow-position",
                        Value::uint(match arrow_position {
                            ConfigPanelItemLauncherArrowPosition::Default => 0,
                            ConfigPanelItemLauncherArrowPosition::North => 1,
                            ConfigPanelItemLauncherArrowPosition::West => 2,
                            ConfigPanelItemLauncherArrowPosition::East => 3,
                            ConfigPanelItemLauncherArrowPosition::South => 4,
                            ConfigPanelItemLauncherArrowPosition::InsideButton => 5,
                        }),
                    ),
                ],
                item_ids
                    .zip(items)
                    .map(|(item_id, item)| match item {
                        ConfigPanelItemLauncherItem::Str(s) => {
                            ConfigFile::Link(PathBuf::from(s))
                        },
                        ConfigPanelItemLauncherItem::Struct(
                            ConfigPanelItemLauncherItemStruct {
                                name,
                                comment,
                                command,
                                icon,
                                startup_notification,
                                run_in_terminal,
                            },
                        ) => ConfigFile::File(ConfigFileFile {
                            path: PathBuf::from(format!(
                                "launcher-{}/{}.desktop",
                                plugin_id, item_id
                            )),
                            contents: Cfg {
                                // TODO: launcher cfg file
                            },
                        }),
                    })
                    .collect(),
            )
        },
        ConfigPanelItem::Whiskermenu(_) => todo!(),
    }
}
