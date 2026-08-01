pub mod autosave;
pub mod logging;
pub mod recents;
pub mod recovery;
pub mod settings;
pub mod state;
pub mod undo;

pub use logging::init_logging;
pub use recents::{RecentEntry, RecentStore};
pub use recovery::{RecoveredDoc, RecoveryManager};
pub use settings::AppSettings;
pub use state::{AppState, OpenDocumentInfo};
pub use undo::{UndoOp, UndoStack};
