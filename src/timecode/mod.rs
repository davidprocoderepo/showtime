//! Timecode SMPTE (HH:MM:SS:FF) e conversões com frame rate e drop-frame.

pub mod conversion;
pub mod model;

pub use conversion::seconds_to_timecode;
pub use model::Timecode;