#![recursion_limit = "256"]
#![warn(
    rust_2018_idioms,
    rust_2021_compatibility,
    rust_2024_compatibility,
    future_incompatible,
    nonstandard_style,
    unused,
    clippy::all,
    clippy::nursery,
    clippy::pedantic
)]
#![allow(
    clippy::default_trait_access,
    clippy::doc_markdown,
    clippy::enum_glob_use,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_ref_mut,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::use_self
)]
#![cfg_attr(
    any(test, feature = "integration-test"),
    allow(clippy::wildcard_imports)
)]

#[macro_use]
extern crate maplit;

pub mod auth;
pub mod cloudflare;
pub mod config;
pub mod database;
pub mod email;
pub mod grpc;
pub mod http;
pub mod model;
pub mod mqtt;
pub mod server;
pub mod store;
pub mod stripe;
pub mod util;
