//! Modelo e gerenciamento de marcadores (cues).

pub mod manager;
pub mod model;

pub use manager::MarkerManager;
pub use model::{Marker, MarkerType};