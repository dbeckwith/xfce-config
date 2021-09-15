#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

pub mod channel;
pub mod config;

use self::{channel::*, config::*};
use std::path::PathBuf;

pub struct ConfigFile {
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
    todo!()
}
