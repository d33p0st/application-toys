//! # toys_macros
//!
//! Procedural macros for the [`toys`](https://docs.rs/application-toys) crate.
//!
//! ## `#[asynchronous]`
//!
//! Transforms every `async fn` inside a `trait` or `impl` block into a function
//! that returns `Pin<Box<dyn Future<Output = T> + Send + Sync + 'lifetime>>`.
//!
//! This makes traits with async methods object-safe (`dyn`-compatible), which is
//! not possible with bare `async fn` in stable Rust today.
//!
//! See [`asynchronous`] for full documentation and examples.

extern crate proc_macro;

#[cfg(feature = "asynchronous-traits")]
mod r#async;

#[cfg(feature = "event")]
mod responsible;

/// Make `async fn` methods in a `trait` or `impl` block dyn-compatible by
/// rewriting them to return `Pin<Box<dyn Future<Output = T> + Send + Sync + 'lt>>`.
///
/// Apply this attribute to a **trait declaration** to rewrite all abstract and
/// default `async fn` signatures. Apply it to an **`impl` block** to additionally
/// wrap each method body in `Box::pin(async move { … })`.
///
/// Non-`async` methods are left untouched in both cases.
///
/// # Lifetime inference
///
/// The future's lifetime bound is derived from the method receiver:
///
/// | Receiver | Future lifetime |
/// |----------|----------------|
/// | `&self` / `&mut self` | `'async_trait` (tied to the borrow) |
/// | `self`, `self: Arc<Self>`, or no receiver | `'static` |
///
/// # Arguments
///
/// Pass optional comma-separated flags inside the attribute:
///
/// | Flag | Effect |
/// |------|--------|
/// | `no_sync` | Remove the `Sync` bound; keep only `Send` |
/// | `local` | Remove both `Send` and `Sync` (single-threaded runtimes) |
/// | `static_lifetime` | Force `'static` even when a borrowed receiver is present |
///
/// # Examples
///
/// ## Trait declaration
///
/// ```rust
/// use toys_macros::asynchronous;
///
/// #[asynchronous]
/// pub trait Worker {
///     async fn run(&self, n: u32) -> String;
/// }
/// ```
///
/// Expands to:
///
/// ```rust,no_run
/// pub trait Worker {
///     fn run<'async_trait>(
///         &'async_trait self,
///         n: u32,
///     ) -> std::pin::Pin<
///         Box<dyn std::future::Future<Output = String> + Send + Sync + 'async_trait>
///     >;
/// }
/// ```
///
/// ## Impl block
///
/// ```rust
/// use toys_macros::asynchronous;
///
/// #[asynchronous]
/// pub trait Worker {
///     async fn run(&self, n: u32) -> String;
/// }
///
/// struct MyWorker;
///
/// #[asynchronous]
/// impl Worker for MyWorker {
///     async fn run(&self, n: u32) -> String {
///         n.to_string()
///     }
/// }
/// ```
///
/// ## Trait with a default implementation
///
/// Concrete methods inside a `trait` block are handled the same way as methods in
/// an `impl` block — the body is wrapped in `Box::pin(async move { … })` automatically:
///
/// ```rust
/// use toys_macros::asynchronous;
///
/// #[asynchronous]
/// pub trait Greeter {
///     async fn name(&self) -> String;
///
///     async fn greet(&self) -> String {
///         format!("Hello, {}!", self.name().await)
///     }
/// }
/// ```
///
/// ## Single-threaded variant
///
/// ```rust
/// use toys_macros::asynchronous;
///
/// #[asynchronous(local)]
/// pub trait LocalWorker {
///     async fn run(&self) -> u32;
/// }
/// ```
///
/// # Notes
///
/// - Reference arguments *other than* the receiver (e.g. `data: &str`) are not given
///   an explicit lifetime automatically. If the future must capture such a reference,
///   annotate the parameter with a named lifetime or use an owned type instead.
/// - `#[asynchronous]` can only be applied to `trait` or `impl` items; applying it
///   elsewhere is a compile error.
#[cfg(feature = "asynchronous-traits")]
#[proc_macro_attribute]
pub fn asynchronous(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    r#async::asynchronous_macro(attr, item)
}

/// Appends a [`tokio::sync::oneshot::Sender<T>`] to enum variants marked with
/// `#[responsible(T)]`, forcing callers to supply a oneshot channel sender.
///
/// Apply this attribute to the **enum definition**. Mark individual variants
/// with `#[responsible(ResponseType)]` as a helper attribute:
///
/// ```rust,ignore
/// use toys_macros::responsible;
///
/// #[responsible]
/// enum Command {
///     Quit,
///     GetValue(String, #[responsible] ...),
///     #[responsible(u32)]
///     Compute(String),   // becomes Compute(String, tokio::sync::oneshot::Sender<u32>)
///     #[responsible(String)]
///     Ping,              // becomes Ping(tokio::sync::oneshot::Sender<String>)
/// }
/// ```
///
/// The caller must create a `tokio::sync::oneshot::channel::<T>()`, pass the
/// `Sender` when constructing the variant, and await the `Receiver` for the
/// response.
#[cfg(feature = "event")]
#[proc_macro_attribute]
pub fn responsible(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    responsible::responsible_macro(attr, item)
}
