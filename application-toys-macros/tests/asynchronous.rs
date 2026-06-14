//! Integration tests for the #[asynchronous] attribute macro.
#![allow(dead_code)]

use toys_macros::asynchronous;

// ── Trait declaration ────────────────────────────────────────────────────────

#[asynchronous]
trait Worker {
    async fn run(&self, n: u32) -> String;
    async fn compute(&mut self, x: i32, y: i32) -> i32;
    // Non-async methods must pass through unchanged
    fn name(&self) -> &str;
    // Static (no receiver) async → 'static
    async fn create() -> Self
    where
        Self: Sized;
}

// ── Impl block ───────────────────────────────────────────────────────────────

struct MyWorker {
    label: String,
}

#[asynchronous]
impl Worker for MyWorker {
    async fn run(&self, n: u32) -> String {
        format!("{}: {n}", self.label)
    }

    async fn compute(&mut self, x: i32, y: i32) -> i32 {
        self.label = format!("computed({x},{y})");
        x + y
    }

    fn name(&self) -> &str {
        &self.label
    }

    async fn create() -> Self {
        MyWorker { label: String::from("new") }
    }
}

// ── no_sync flag ─────────────────────────────────────────────────────────────

#[asynchronous(no_sync)]
trait SendOnly {
    async fn go(&self) -> u32;
}

struct S;

#[asynchronous(no_sync)]
impl SendOnly for S {
    async fn go(&self) -> u32 {
        42
    }
}

// ── local flag ───────────────────────────────────────────────────────────────

#[asynchronous(local)]
trait LocalTrait {
    async fn compute(&self) -> i32;
}

struct L;

#[asynchronous(local)]
impl LocalTrait for L {
    async fn compute(&self) -> i32 {
        7
    }
}

// ── static_lifetime flag ─────────────────────────────────────────────────────

#[asynchronous(static_lifetime)]
trait StaticTrait {
    async fn ping(&self) -> bool;
}

struct St;

#[asynchronous(static_lifetime)]
impl StaticTrait for St {
    async fn ping(&self) -> bool {
        true
    }
}

// ── Runtime smoke test ───────────────────────────────────────────────────────

#[test]
fn test_basic_expansion() {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let worker = MyWorker { label: String::from("test") };
    let result = rt.block_on(worker.run(5));
    assert_eq!(result, "test: 5");

    let mut worker2 = MyWorker { label: String::from("initial") };
    let sum = rt.block_on(worker2.compute(3, 4));
    assert_eq!(sum, 7);
    assert_eq!(worker2.name(), "computed(3,4)");

    let created = rt.block_on(MyWorker::create());
    assert_eq!(created.name(), "new");
}

#[test]
fn test_no_sync() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let s = S;
    let v = rt.block_on(s.go());
    assert_eq!(v, 42);
}

#[test]
fn test_local() {
    // LocalTrait futures are not Send, so run on current-thread runtime
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let l = L;
    let v = rt.block_on(l.compute());
    assert_eq!(v, 7);
}

#[test]
fn test_static_lifetime() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let st = St;
    assert!(rt.block_on(st.ping()));
}
