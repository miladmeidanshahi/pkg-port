use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use zbus::blocking::{connection::Builder, Connection, Proxy};

struct TrayItem {
    activate: Sender<()>,
    /// Icon in StatusNotifierItem `IconPixmap` format (ARGB32, network byte
    /// order), so the tray shows the app icon without any system install.
    pixmap: (i32, i32, Vec<u8>),
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl TrayItem {
    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        "pkgport"
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        "PKGPort"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    fn icon_name(&self) -> &str {
        "pkgport"
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> (i32, i32, Vec<u8>) {
        self.pixmap.clone()
    }

    #[zbus(property)]
    fn window_id(&self) -> u32 {
        0
    }

    fn activate(&self, _x: i32, _y: i32) {
        let _ = self.activate.send(());
    }

    fn secondary_activate(&self, x: i32, y: i32) {
        self.activate(x, y);
    }
}

/// The embedded app icon as ARGB32 bytes for `IconPixmap`.
fn app_pixmap() -> (i32, i32, Vec<u8>) {
    let icon = image::load_from_memory(include_bytes!("../assets/icon-32.png"))
        .expect("bundled tray icon decodes");
    let icon = icon.to_rgba8();
    let (width, height) = (icon.width() as i32, icon.height() as i32);
    let data = icon
        .pixels()
        .flat_map(|p| [p.0[3], p.0[0], p.0[1], p.0[2]])
        .collect();
    (width, height, data)
}

pub fn start() -> (Sender<()>, Receiver<()>) {
    let (activate_sender, activate_receiver) = mpsc::channel();
    let (stop_sender, stop_receiver) = mpsc::channel();
    thread::spawn(move || match register(activate_sender) {
        Ok(_connection) => {
            let _ = stop_receiver.recv();
        }
        Err(error) => eprintln!("System tray unavailable: {error}"),
    });
    (stop_sender, activate_receiver)
}

fn register(sender: Sender<()>) -> zbus::Result<Connection> {
    let name = format!(
        "org.freedesktop.StatusNotifierItem-{}-1",
        std::process::id()
    );
    let connection = Builder::session()?
        .serve_at(
            "/StatusNotifierItem",
            TrayItem {
                activate: sender,
                pixmap: app_pixmap(),
            },
        )?
        .name(name.as_str())?
        .build()?;
    let watcher = Proxy::new(
        &connection,
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher",
    )?;
    watcher.call_method("RegisterStatusNotifierItem", &(name.as_str(),))?;
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_activation_reaches_the_window() {
        let (sender, receiver) = mpsc::channel();
        TrayItem {
            activate: sender,
            pixmap: app_pixmap(),
        }
        .activate(0, 0);
        assert!(receiver.try_recv().is_ok());
    }

    #[test]
    fn pixmap_is_argb32_network_order() {
        let (width, height, data) = app_pixmap();
        assert_eq!((width, height), (32, 32));
        assert_eq!(data.len(), (32 * 32 * 4) as usize);
        let source = image::load_from_memory(include_bytes!("../assets/icon-32.png"))
            .unwrap()
            .to_rgba8();
        // Every pixel is [alpha, red, green, blue].
        for (argb, rgba) in data.as_chunks::<4>().0.iter().zip(source.pixels()) {
            assert_eq!(*argb, [rgba.0[3], rgba.0[0], rgba.0[1], rgba.0[2]]);
        }
    }
}
