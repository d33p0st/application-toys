//! Typed global broadcast event bus.
//!
//! This module provides a process-wide publish/subscribe system built on
//! [`tokio::sync::broadcast`]. Events are keyed by their Rust type — one channel
//! per type, created lazily on first access and shared for the lifetime of the process.
//!
//! ## Core types
//!
//! | Type | Role |
//! |------|------|
//! | [`EventChannel<E>`] | The underlying broadcast transport for event type `E` |
//! | [`EventHandler<E>`] | Trait implemented by types that react to events of type `E` |
//! | [`EventLoop<E>`] | Registers handlers and starts their background receive tasks |
//!
//! ## Usage pattern
//!
//! 1. Define an event type (typically an `enum`).
//! 2. Implement [`EventHandler<E>`] on each consumer struct, annotated with
//!    [`#[asynchronous]`](crate::asynchronous).
//! 3. Register all handlers at startup via [`EventLoop::dispatch`].
//! 4. Publish events from anywhere with [`event::<E>().dispatch(…)`](event).
//!
//! ## Example
//!
//! ```no_run
//! use toys::event::{event, EventHandler, EventLoop};
//! use toys::asynchronous;
//! use std::sync::Arc;
//! use std::sync::atomic::{AtomicU64, Ordering};
//!
//! pub enum AppEvent { Increment(u64) }
//!
//! struct Counter {
//!     value: AtomicU64,
//! }
//!
//! #[asynchronous]
//! impl EventHandler<AppEvent> for Counter {
//!     async fn handle(self: Arc<Self>, event: AppEvent) {
//!         if let AppEvent::Increment(n) = event {
//!             self.value.fetch_add(n, Ordering::Relaxed);
//!         }
//!     }
//! }
//!
//! # tokio_test::block_on(async {
//! let counter = Arc::new(Counter { value: AtomicU64::new(0) });
//! // if you dont clone counter, it moves it.
//! let handles = EventLoop::<AppEvent>::new().dispatch(&[counter.clone()]).await;
//!
//! event::<AppEvent>().dispatch(AppEvent::Increment(1)).await.unwrap();
//! event::<AppEvent>().dispatch(AppEvent::Increment(30)).await.unwrap();
//!
//! // shut down the handler's background task
//! handles[0].1.send(()).unwrap();
//! handles[0].0.await.unwrap().unwrap();
//! println!("counter = {}", counter.value.load(Ordering::Relaxed));
//! # });
//! ```
//! Example Output:
//! ```
//! counter = 31
//! ```

use crate::asynchronous;
use anyhow::Result;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::broadcast;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

static EventTypeRegistry: OnceLock<Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>> =
    OnceLock::new();

/// Return the process-global [`EventChannel`] for event type `Type`, creating it
/// on first call.
///
/// Every call with the same `Type` returns the same `Arc<EventChannel<Type>>`, so
/// all senders and all subscribers share one broadcast bus per event type.
///
/// Subscriptions are managed internally by [`EventHandler`] — call
/// [`EventLoop::dispatch`] to register a handler rather than subscribing directly.
///
/// # Type parameters
///
/// - `Type` — the event payload. Must be [`Clone`] + [`Send`] + [`Sync`] + `'static`
///   because it is broadcast across threads.
///
/// # Returns
///
/// `Arc<`[`EventChannel`]`<Type>>` whose [`dispatch`](EventChannel::dispatch) method
/// publishes an event to every active subscriber.
///
/// # Example
///
/// ```no_run
/// use toys::event::{event, EventHandler, EventLoop};
/// use toys::asynchronous;
/// use std::sync::Arc;
///
/// pub enum Events {
///     Tick,
///     Tock,
/// }
///
/// struct Listener;
///
/// #[asynchronous]
/// impl EventHandler<Events> for Listener {
///     async fn handle(self: Arc<Self>, event: Events) {
///         match event {
///             Events::Tick => println!("tick"),
///             Events::Tock => println!("tock"),
///         }
///     }
/// }
///
/// # tokio_test::block_on(async {
/// let listener = Arc::new(Listener);
/// // if you dont clone listener, it moves it.
/// EventLoop::<Events>::new().dispatch(&[listener.clone()]).await;
///
/// event::<Events>().dispatch(Events::Tick).await.unwrap();
/// event::<Events>().dispatch(Events::Tock).await.unwrap();
/// # });
/// ```
///
/// Example Output:
/// ```
/// tick
/// tock
/// ```
pub fn event<Type>() -> Arc<EventChannel<Type>>
where
    Type: Clone + Send + Sync + 'static,
{
    let map = EventTypeRegistry.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().unwrap();
    let type_id = TypeId::of::<Type>();
    if let Some(channel) = guard.get(&type_id) {
        channel.clone().downcast::<EventChannel<Type>>().unwrap()
    } else {
        let channel = Arc::new(EventChannel::<Type>::new());
        guard.insert(type_id, channel.clone() as Arc<dyn Any + Send + Sync>);
        channel
    }
}

/// Broadcast transport for a single event type.
///
/// `EventChannel<Type>` is the underlying message bus for one event type. It holds a
/// [`tokio::sync::broadcast`] sender with a fixed capacity of 64 messages and exposes
/// only the ability to publish — subscriptions are managed by [`EventHandler`].
///
/// Obtain an instance via [`event::<Type>()`](event), which guarantees at most one
/// channel per type for the lifetime of the process.
pub struct EventChannel<Type: Clone> {
    sender: broadcast::Sender<Type>,
}

impl<Type: Clone> EventChannel<Type> {
    /// Create a standalone `EventChannel` with a broadcast capacity of 64 messages.
    ///
    /// In most cases you should call [`event::<Type>()`](event) instead, which
    /// returns the shared process-global instance rather than creating a new one.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(64);
        Self { sender }
    }

    fn subscribe(&self) -> broadcast::Receiver<Type> {
        self.sender.subscribe()
    }

    /// Broadcast `message` to every active subscriber.
    ///
    /// Subscribers that have fallen behind by more than the channel capacity are
    /// silently skipped by the underlying broadcast implementation; they will
    /// receive a [`broadcast::error::RecvError::Lagged`] error on their next receive.
    ///
    /// # Errors
    ///
    /// Returns an error if there are no active receivers (all subscribers have been
    /// dropped or the channel has been closed).
    pub async fn dispatch(&self, message: Type) -> Result<()> {
        self.sender
            .send(message)
            .map(|_| ())
            .map_err(|_| anyhow::anyhow!("broadcast channel closed"))
    }
}

/// Implemented by types that react to events of type `Type`.
///
/// Apply [`#[asynchronous]`](crate::asynchronous) to the `impl` block so the
/// `async fn handle` signature is correctly transformed. Then register an `Arc` of
/// your type with [`EventLoop::dispatch`] to start receiving events.
///
/// The trait provides a default `_generate` method that wires the subscription and
/// spawns a background receive task; you only need to implement [`handle`](Self::handle).
///
/// # Type parameters
///
/// - `Type` — the event enum or struct this handler reacts to.
///
/// # Example
///
/// ```no_run
/// use toys::event::{event, EventHandler, EventLoop};
/// use toys::asynchronous;
/// use std::sync::Arc;
///
/// pub enum Cmd { Greet(String) }
///
/// struct Greeter;
///
/// #[asynchronous]
/// impl EventHandler<Cmd> for Greeter {
///     async fn handle(self: Arc<Self>, event: Cmd) {
///         if let Cmd::Greet(name) = event {
///             println!("Hello, {name}!");
///         }
///     }
/// }
///
/// # tokio_test::block_on(async {
/// let g = Arc::new(Greeter) as Arc<dyn EventHandler<Cmd>>;
/// EventLoop::<Cmd>::new().dispatch(&[g]).await;
/// event::<Cmd>().dispatch(Cmd::Greet("world".into())).await.unwrap();
/// # });
/// ```
#[asynchronous]
pub trait EventHandler<Type>: Send + Sync {
    /// Handle one event received from the global [`EventChannel`].
    ///
    /// Receives `Arc<Self>` rather than `&self` or `&mut self` so the handler can be
    /// shared across concurrent tasks without exclusive access. Heavy per-call state
    /// should be stored behind an inner `Mutex` or `RwLock` on the implementing type.
    ///
    /// The `where` bounds enforce that both the implementor and the event type are
    /// safe to transfer across thread boundaries.
    async fn handle(self: Arc<Self>, event: Type) -> ()
    where
        Self: Send + Sync + 'static,
        Type: Clone + Send + Sync + 'static;

    /// Subscribe to the global channel and spawn a background task that forwards
    /// each event to [`handle`](Self::handle).
    ///
    /// Returns a `(`[`JoinHandle`](tokio::task::JoinHandle)`, `[`UnboundedSender`]`<()>)`
    /// pair. Send any value on the `UnboundedSender` to signal graceful shutdown; then
    /// `.await` the `JoinHandle` to confirm the task has exited.
    ///
    /// Called automatically by [`EventLoop::dispatch`] — you do not need to invoke
    /// this directly.
    #[doc(hidden)]
    async fn _generate(
        self: Arc<Self>,
    ) -> (tokio::task::JoinHandle<Result<()>>, UnboundedSender<()>)
    where
        Self: Send + Sync + 'static,
        Type: Clone + Send + Sync + 'static,
    {
        let (quit_tx, mut quit_rx) = unbounded_channel::<()>();
        let mut event_rx = event::<Type>().subscribe();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = event_rx.recv() => {
                        match result {
                            event => Self::handle(self.clone(), event?).await,
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        }
                    }
                    _ = quit_rx.recv() => break,
                }
            }
            Ok(())
        });
        (handle, quit_tx)
    }
}

/// Registers [`EventHandler`] implementations and starts their background receive tasks.
///
/// `EventLoop<Type>` is the entry point for wiring one or more handlers into the
/// global event bus for a given event type. Each call to [`dispatch`](Self::dispatch)
/// spawns one Tokio task per handler; those tasks run until a quit signal is sent or
/// the program exits.
///
/// # Type parameters
///
/// - `Type` — the event type shared by all handlers registered with this loop.
///
/// # Example
///
/// ```no_run
/// use toys::event::{event, EventHandler, EventLoop};
/// use toys::asynchronous;
/// use std::sync::Arc;
///
/// pub enum Job { Run }
///
/// struct Worker;
///
/// #[asynchronous]
/// impl EventHandler<Job> for Worker {
///     async fn handle(self: Arc<Self>, _event: Job) {
///         println!("working");
///     }
/// }
///
/// # tokio_test::block_on(async {
/// let w = Arc::new(Worker) as Arc<dyn EventHandler<Job>>;
/// let handles = EventLoop::<Job>::new().dispatch(&[w]).await;
///
/// event::<Job>().dispatch(Job::Run).await.unwrap();
///
/// // graceful shutdown
/// handles[0].1.send(()).unwrap();
/// handles[0].0.await.unwrap().unwrap();
/// # });
/// ```
pub struct EventLoop<Type>
where
    Type: Clone + Send + Sync + 'static,
{
    phantom: std::marker::PhantomData<Type>,
}

impl<Type> EventLoop<Type>
where
    Type: Clone + Send + Sync + 'static,
{
    /// Create a new `EventLoop` for event type `Type`.
    pub fn new() -> Self {
        Self {
            phantom: std::marker::PhantomData,
        }
    }

    /// Subscribe each handler in `objects` to the global [`EventChannel`] and spawn
    /// one background task per handler.
    ///
    /// # Parameters
    ///
    /// - `objects` — a slice of `Arc<dyn EventHandler<Type>>`. Each entry receives
    ///   its own subscription and independent task.
    ///
    /// # Returns
    ///
    /// A [`Vec`] with one `(`[`JoinHandle`](tokio::task::JoinHandle)`<`[`Result`]`<()>>, `[`UnboundedSender`]`<()>)`
    /// per handler, in the same order as `objects`. Send `()` on the `UnboundedSender`
    /// to shut down a specific handler's task; await the `JoinHandle` to confirm it
    /// has exited cleanly.
    pub async fn dispatch(
        &self,
        objects: &[Arc<dyn EventHandler<Type>>],
    ) -> Vec<(tokio::task::JoinHandle<Result<()>>, UnboundedSender<()>)> {
        let mut vector = Vec::new();
        for object in objects {
            let this = object.clone();
            vector.push(this._generate().await);
        }
        vector
    }
}
