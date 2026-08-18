//! Interface gráfica (eframe/egui). A UI é em português.
//!
//! A UI CHAMA o core (`audio`, `markers`, `timecode`, `export`, `project`,
//! `live`) — o core jamais importa `egui`.

pub mod app;
pub mod marker_panel;
pub mod settings;
pub mod timeline;
pub mod transport;