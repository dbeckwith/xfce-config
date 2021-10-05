use anyhow::{Context, Result};

pub struct DBus {
    proxy: gio::DBusProxy,
}

impl DBus {
    pub fn new(destination: &'static str, path: &'static str) -> Result<Self> {
        let proxy = gio::DBusProxy::for_bus_sync::<gio::Cancellable>(
            gio::BusType::Session,
            gio::DBusProxyFlags::NONE,
            None,
            destination,
            path,
            destination,
            None,
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
    ) -> Result<()> {
        let args = args.to_variant();
        assert!(args.is_container());
        gio::prelude::DBusProxyExt::call_sync::<gio::Cancellable>(
            &self.proxy,
            method,
            Some(&args),
            gio::DBusCallFlags::NONE,
            -1,
            None,
        )
        .with_context(|| format!("{}{}", method, args.to_string()))?;
        Ok(())
    }
}
