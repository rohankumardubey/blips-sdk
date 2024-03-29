#![doc = include_str!("../README.md")]

mod client;
mod client_generated;
mod core;
pub mod graphql;

pub use crate::core::*;
pub use client::*;
