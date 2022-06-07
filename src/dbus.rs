use anyhow::{Context, Result};
use std::borrow::Cow;

pub struct DBus {
    proxy: gio::DBusProxy,
}

impl DBus {
    pub fn new(destination: &'static str, path: &'static str) -> Result<Self> {
        let proxy = gio::DBusProxy::for_bus_sync(
            gio::BusType::Session,
            gio::DBusProxyFlags::NONE,
            None,
            destination,
            path,
            destination,
            None::<&gio::Cancellable>,
        )
        .with_context(|| {
            format!("error creating dbus proxy for {}", destination)
        })?;
        Ok(Self { proxy })
    }

    pub fn call(
        &mut self,
        method: &'static str,
        args: impl glib::variant::ToVariant,
    ) -> Result<glib::Variant> {
        self.call_inner(method, Some(args.to_variant()))
    }

    pub fn call_no_args(
        &mut self,
        method: &'static str,
    ) -> Result<glib::Variant> {
        self.call_inner(method, None)
    }

    fn call_inner(
        &mut self,
        method: &'static str,
        args: Option<glib::variant::Variant>,
    ) -> Result<glib::Variant> {
        gio::prelude::DBusProxyExt::call_sync(
            &self.proxy,
            method,
            args.as_ref(),
            gio::DBusCallFlags::NONE,
            -1,
            None::<&gio::Cancellable>,
        )
        .with_context(|| {
            format!(
                "{}{}",
                method,
                args.as_ref()
                    .map(ToString::to_string)
                    .map_or(Cow::Borrowed("()"), Cow::Owned)
            )
        })
    }
}
