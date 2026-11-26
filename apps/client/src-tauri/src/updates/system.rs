//! The steps of an update that leave the app: stopping the local database, running the installer and
//! restarting. The tests can't run them (the installer replaces this program), so they're kept to these
//! few lines and checked by updating a real installed app (`docs/deployment/release-process.md`).

use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Error, Update, UpdaterBuilder};

use super::System;
use crate::backend::Backend;

/// The installed desktop app.
pub struct Desktop(pub AppHandle);

impl System for Desktop {
    fn prepare(&self, builder: UpdaterBuilder) -> UpdaterBuilder {
        let app = self.0.clone();
        builder.on_before_exit(move || app.state::<Backend>().stop())
    }

    fn install(&self, update: &Update, bytes: &[u8]) -> Result<(), Error> {
        update.install(bytes)
    }

    fn restart(&self) {
        self.0.state::<Backend>().stop();
        self.0.restart();
    }
}
