// SPDX-License-Identifier: MIT OR Apache-2.0
pub mod logging;
pub mod recents;
pub mod recovery;
pub mod settings;
pub mod state;

pub use logging::{audit_event, init_logging};
pub use recents::{RecentEntry, RecentStore};
pub use recovery::{RecoveredDoc, RecoveryManager};
pub use settings::{sanitize_author_name, AppSettings, AuthorProfile};
pub use state::{AppState, OpenDocumentInfo};
