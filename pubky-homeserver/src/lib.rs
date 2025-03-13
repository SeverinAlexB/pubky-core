#![doc = include_str!("../README.md")]
//!

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![cfg_attr(any(), deny(clippy::unwrap_used))]


mod core;
mod api;
mod persistence;
mod services;

pub use api::homeserver::Homeserver;
pub use api::homeserver::HomeserverBuilder;
