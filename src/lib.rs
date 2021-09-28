#![warn(rust_2018_idioms, clippy::all)]
#![deny(clippy::correctness)]

pub mod cfg;
pub mod channel;

use self::{cfg::*, channel::*};
use serde::Deserialize;
use std::{borrow::Cow, path::Path};

#[derive(Debug, Deserialize)]
pub struct XfceConfig<'a> {
    pub channels: Vec<Channel<'a>>,
    pub config_files: Vec<ConfigFile<'a>>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigFile<'a> {
    pub file: ConfigFileFile<'a>,
    pub plugin: ConfigFilePlugin<'a>,
}

#[derive(Debug, Deserialize)]
pub enum ConfigFileFile<'a> {
    Rc(Cfg<'a>),
    DesktopDir(DesktopDir<'a>),
}

#[derive(Debug, Deserialize)]
pub struct DesktopDir<'a> {
    pub files: Vec<DesktopFile<'a>>,
}

#[derive(Debug, Deserialize)]
pub struct DesktopFile<'a> {
    pub id: u32,
    pub content: DesktopFileContent<'a>,
}

#[derive(Debug, Deserialize)]
pub enum DesktopFileContent<'a> {
    Cfg(Cfg<'a>),
    Link(Cow<'a, Path>),
}

#[derive(Debug, Deserialize)]
pub struct ConfigFilePlugin<'a> {
    pub id: u32,
    pub r#type: Cow<'a, str>,
}
