use tauri::{
  plugin::{Builder, TauriPlugin},
  Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Libsql;
#[cfg(mobile)]
use mobile::Libsql;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the libsql APIs.
pub trait LibsqlExt<R: Runtime> {
  fn libsql(&self) -> &Libsql<R>;
}

impl<R: Runtime, T: Manager<R>> crate::LibsqlExt<R> for T {
  fn libsql(&self) -> &Libsql<R> {
    self.state::<Libsql<R>>().inner()
  }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
  Builder::new("libsql")
    .invoke_handler(tauri::generate_handler![commands::ping])
    .setup(|app, api| {
      #[cfg(mobile)]
      let libsql = mobile::init(app, api)?;
      #[cfg(desktop)]
      let libsql = desktop::init(app, api)?;
      app.manage(libsql);
      Ok(())
    })
    .build()
}
