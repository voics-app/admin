//! HTTP interface: routes, auth middleware, DTO mapping, SPA serving.

pub mod auth;
pub mod dto;
pub mod error;
pub mod handlers;
pub mod ops;
pub mod router;
pub mod spa;
pub mod state;

#[cfg(test)]
mod tests;
