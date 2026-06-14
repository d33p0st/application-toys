//! # toys
//!
//! Lightweight application primitives for async Rust, built on [Tokio].
//!
//! Each primitive lives behind a feature flag so you only compile what you need.
//!
//! ## Feature flags
//!
//! | Flag | Default | Enables |
//! |------|:-------:|---------|
//! | `event` | | Typed global broadcast event bus and the [`asynchronous`] attribute macro |
//!
//! ## Event system
//!
//! The event system decouples producers and consumers of typed events through a
//! process-wide broadcast channel. Any number of handlers can subscribe to the same
//! event type independently.
//!
//! ```text
//!                     event::<E>().dispatch(e)
//!                              │
//!                              ▼
//!                   ┌──────────────────────┐
//!                   │    EventChannel<E>   │  ← one per type, process-global
//!                   └──────────┬───────────┘
//!                              │ broadcast
//!               ┌──────────────┼──────────────┐
//!               ▼              ▼              ▼
//!          Handler A       Handler B       Handler C
//! ```
//!
//! ### Minimal example
//!
//! ```no_run
//! use toys::event::{event, EventHandler, EventLoop};
//! use toys::asynchronous;
//! use std::sync::Arc;
//!
//! pub enum Signal { Ping }
//!
//! struct Logger;
//!
//! #[asynchronous]
//! impl EventHandler<Signal> for Logger {
//!     async fn handle(self: Arc<Self>, _event: Signal) {
//!         println!("ping received");
//!     }
//! }
//!
//! # tokio_test::block_on(async {
//! let logger = Arc::new(Logger) as Arc<dyn EventHandler<Signal>>;
//! EventLoop::<Signal>::new().dispatch(&[logger]).await;
//! event::<Signal>().dispatch(Signal::Ping).await.unwrap();
//! # });
//! ```
//!
//! [`EventHandler`]: event::EventHandler
//! [`EventLoop`]: event::EventLoop
//! [`EventLoop::dispatch`]: event::EventLoop::dispatch
//! [Tokio]: https://tokio.rs

#![allow(
    dead_code,
    unused,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]

/// Typed global broadcast event bus.
///
/// See [`event::event`] to obtain a channel, [`event::EventHandler`] to react to
/// events, and [`event::EventLoop`] to register handlers.
#[cfg(feature = "event")]
pub mod event;

/// Transforms `async fn` methods in `trait` and `impl` blocks into boxed,
/// dyn-compatible futures.
///
/// Re-exported from `toys_macros`. See that crate's documentation for supported
/// arguments (`no_sync`, `local`, `static_lifetime`).
#[cfg(feature = "event")]
pub use toys_macros::asynchronous;
