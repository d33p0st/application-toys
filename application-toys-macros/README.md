# application-toys-macros

Procedural macros for the [`application-toys`](https://crates.io/crates/application-toys) crate.

## Overview

This crate provides the `#[asynchronous]` attribute macro, which makes `async fn` methods in `trait` and `impl` blocks **dyn-compatible** (object-safe) by rewriting them to return `Pin<Box<dyn Future<Output = T> + Send + Sync + 'lt>>`.

> **Note:** You normally do not depend on this crate directly — use `application-toys` with the `event` feature instead, which re-exports `#[asynchronous]` as `toys::asynchronous`.

## Usage

```toml
[dependencies]
application-toys-macros = "0.0.1"
```

### Trait declaration

```rust
use toys_macros::asynchronous;

#[asynchronous]
pub trait Worker {
    async fn run(&self, n: u32) -> String;
}
```

Expands to:

```rust
pub trait Worker {
    fn run<'async_trait>(
        &'async_trait self,
        n: u32,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = String> + Send + Sync + 'async_trait>
    >;
}
```

### Impl block

```rust
use toys_macros::asynchronous;

#[asynchronous]
pub trait Worker {
    async fn run(&self, n: u32) -> String;
}

struct MyWorker;

#[asynchronous]
impl Worker for MyWorker {
    async fn run(&self, n: u32) -> String {
        n.to_string()
    }
}
```

### Trait with a default implementation

```rust
use toys_macros::asynchronous;

#[asynchronous]
pub trait Greeter {
    async fn name(&self) -> String;

    async fn greet(&self) -> String {
        format!("Hello, {}!", self.name().await)
    }
}
```

### Optional flags

Pass comma-separated flags inside the attribute:

| Flag | Effect |
|------|--------|
| `no_sync` | Remove the `Sync` bound; keep only `Send` |
| `local` | Remove both `Send` and `Sync` (single-threaded runtimes) |
| `static_lifetime` | Force `'static` even when a borrowed receiver is present |

```rust
use toys_macros::asynchronous;

// Single-threaded runtimes
#[asynchronous(local)]
pub trait LocalWorker {
    async fn run(&self) -> u32;
}
```

## Lifetime inference

| Receiver | Future lifetime |
|----------|----------------|
| `&self` / `&mut self` | `'async_trait` (tied to the borrow) |
| `self`, `self: Arc<Self>`, or no receiver | `'static` |

## Features

| Feature | Default | Description |
|---------|:-------:|-------------|
| `asynchronous-traits` | | Enables the `#[asynchronous]` macro (activated automatically by `application-toys`) |

## License

MIT — see [LICENSE](LICENSE).
