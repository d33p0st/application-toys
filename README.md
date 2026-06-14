# application-toys

[![crates.io](https://img.shields.io/crates/v/application-toys.svg)](https://crates.io/crates/application-toys)
[![docs.rs](https://docs.rs/application-toys/badge.svg)](https://docs.rs/application-toys)
[![license](https://img.shields.io/crates/l/application-toys.svg)](LICENSE)

Lightweight application primitives for async Rust, built on [Tokio](https://tokio.rs).

Each primitive lives behind a feature flag so you only compile what you need. More tools will be added in future releases.

## Feature flags

| Flag | Default | Enables |
|------|:-------:|---------|
| `event` | | Typed global broadcast event bus and the `#[asynchronous]` attribute macro |

```toml
[dependencies]
application-toys = { version = "0.0.1", features = ["event"] }
```

---

## Event system (`event`)

A process-wide, type-keyed publish/subscribe bus built on `tokio::sync::broadcast`. One channel exists per event type; all senders and handlers share it automatically.

```text
                event::<E>().dispatch(e)
                         │
                         ▼
              ┌──────────────────────┐
              │    EventChannel<E>   │  ← one per type, process-global
              └──────────┬───────────┘
                         │ broadcast
             ┌───────────┼───────────┐
             ▼           ▼           ▼
        Handler A    Handler B    Handler C
```

### Quick start

```rust
use toys::event::{event, EventHandler, EventLoop};
use toys::asynchronous;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub enum AppEvent { Increment(u64) }

struct Counter {
    value: AtomicU64,
}

#[asynchronous]
impl EventHandler<AppEvent> for Counter {
    async fn handle(self: Arc<Self>, event: AppEvent) {
        if let AppEvent::Increment(n) = event {
            self.value.fetch_add(n, Ordering::Relaxed);
        }
    }
}

#[tokio::main]
async fn main() {
    let counter = Arc::new(Counter { value: AtomicU64::new(0) });

    let handles = EventLoop::<AppEvent>::new()
        .dispatch(&[counter.clone()])
        .await;

    event::<AppEvent>().dispatch(AppEvent::Increment(1)).await.unwrap();
    event::<AppEvent>().dispatch(AppEvent::Increment(30)).await.unwrap();

    // graceful shutdown
    handles[0].1.send(()).unwrap();
    handles[0].0.await.unwrap().unwrap();

    println!("counter = {}", counter.value.load(Ordering::Relaxed));
    // counter = 31
}
```

### How it works

1. **Define** an event type — any `Clone + Send + Sync + 'static` type works, typically an `enum`.
2. **Implement** `EventHandler<E>` on your consumer struct, annotated with `#[asynchronous]`.
3. **Register** handlers at startup via `EventLoop::dispatch`, which spawns one background Tokio task per handler.
4. **Publish** from anywhere with `event::<E>().dispatch(value).await`.

### `#[asynchronous]`

The `#[asynchronous]` attribute (re-exported from `application-toys-macros`) makes `async fn` methods in `trait` and `impl` blocks dyn-compatible by rewriting them to return `Pin<Box<dyn Future<Output = T> + Send + Sync + 'lt>>`.

It accepts optional flags:

| Flag | Effect |
|------|--------|
| `no_sync` | Remove the `Sync` bound; keep only `Send` |
| `local` | Remove both `Send` and `Sync` (single-threaded runtimes) |
| `static_lifetime` | Force `'static` even when a borrowed receiver is present |

See [`application-toys-macros`](https://docs.rs/application-toys-macros) for full documentation.

---

## License

MIT — see [LICENSE](LICENSE).
