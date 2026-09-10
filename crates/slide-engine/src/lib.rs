pub mod deck;
pub mod element;
pub mod theme;

pub use deck::{DeckModel, Slide, SlideCommentModel};
pub use element::{ElementKind, SlideElement, TableMerge};
pub use theme::SlideTheme;
