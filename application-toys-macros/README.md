# application-toys-macros

Procedural macros for the [`application-toys`](https://crates.io/crates/application-toys) crate.

## Overview

This crate provides two attribute macros:

- **`#[asynchronous]`** — makes `async fn` methods in `trait` and `impl` blocks **dyn-compatible** by rewriting them to return `Pin<Box<dyn Future<Output = T> + Send + Sync + 'lt>>`.
- **`#[responsible]`** — appends a `tokio::sync::oneshot::Sender<T>` to marked enum variants, enforcing a request/response ownership pattern at the type level.

> **Note:** You normally do not depend on this crate directly — use `application-toys` with the `event` feature instead, which activates both macros.

## Features

| Feature | Default | Description |
|---------|:-------:|-------------|
| `asynchronous-traits` | | Enables `#[asynchronous]` |
| `event` | | Enables `#[responsible]` |

---

## `#[asynchronous]`

Apply to a **trait** or **impl** block to make every `async fn` inside it return a boxed, dyn-compatible future.

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

#[asynchronous(local)]
pub trait LocalWorker {
    async fn run(&self) -> u32;
}
```

### Lifetime inference

| Receiver | Future lifetime |
|----------|----------------|
| `&self` / `&mut self` | `'async_trait` (tied to the borrow) |
| `self`, `self: Arc<Self>`, or no receiver | `'static` |

---

## `#[responsible]`

Apply to an **enum**. Mark individual variants with `#[responsible(ResponseType)]` to append a `tokio::sync::oneshot::Sender<ResponseType>` to that variant's fields.

This forces every caller constructing the variant to create a oneshot channel and pass the sender in, while retaining the receiver to await the response.

### Example

```rust
use toys_macros::responsible;

#[responsible]
enum Command {
    Shutdown,
    #[responsible(String)]
    GetName,
    #[responsible(u32)]
    Compute(String, String, u32),
}
```

Expands to:

```rust
enum Command {
    Shutdown,
    GetName(tokio::sync::oneshot::Sender<String>),
    Compute(String, String, u32, tokio::sync::oneshot::Sender<u32>),
}
```

### Caller pattern

```rust
let (tx, rx) = tokio::sync::oneshot::channel::<u32>();
sender.send(Command::Compute("a".into(), "b".into(), 1, tx)).unwrap();
let result = rx.await.unwrap();
```

### Constraints

- Only tuple (unnamed-field) and unit variants are supported. Named-field variants produce a compile error.
- Multiple `#[responsible]` attributes on the same variant — only the first is processed.

---

## License

MIT — see [LICENSE](LICENSE).
