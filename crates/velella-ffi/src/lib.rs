use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};
use tokio::task::JoinHandle;

uniffi::setup_scaffolding!();

/// The one process-wide runtime. Exported async fns spawn onto it and await the
/// JoinHandle, so bodies run on `velella-rt` threads regardless of who polls the
/// returned future. (`uniffi::export(async_runtime = "tokio")` is deliberately
/// not used: async-compat polls the body on the caller's thread.)
fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        Builder::new_multi_thread()
            .thread_name("velella-rt")
            .enable_time()
            .build()
            .expect("failed to build tokio runtime")
    })
}

/// Name of the thread the body ran on.
#[uniffi::export]
pub async fn runtime_probe() -> String {
    runtime()
        .spawn(async { std::thread::current().name().unwrap_or("").to_owned() })
        .await
        .expect("probe task failed")
}

#[uniffi::export(with_foreign)]
pub trait TickListener: Send + Sync {
    fn on_tick(&self, count: u64);
}

/// Dropping this aborts the tick task.
#[derive(uniffi::Object)]
pub struct Subscription(JoinHandle<()>);

impl Drop for Subscription {
    fn drop(&mut self) {
        self.0.abort();
        eprintln!("velella: subscription dropped");
    }
}

#[uniffi::export]
pub fn subscribe_ticks(listener: Arc<dyn TickListener>) -> Arc<Subscription> {
    start_ticks(listener, Duration::from_secs(1))
}

pub fn start_ticks(listener: Arc<dyn TickListener>, interval: Duration) -> Arc<Subscription> {
    let task = runtime().spawn(async move {
        let mut count = 0u64;
        let mut timer = tokio::time::interval(interval);
        timer.tick().await; // first tick completes immediately
        loop {
            timer.tick().await;
            count += 1;
            listener.on_tick(count);
        }
    });
    Arc::new(Subscription(task))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn async_runs_on_owned_runtime() {
        let name = futures::executor::block_on(runtime_probe());
        assert!(name.starts_with("velella-rt"), "ran on {name:?}");
    }

    struct Counter(Arc<AtomicU64>);
    impl TickListener for Counter {
        fn on_tick(&self, _count: u64) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn dropping_handle_stops_ticks() {
        let interval = Duration::from_millis(20);
        let ticks = Arc::new(AtomicU64::new(0));
        let handle = start_ticks(Arc::new(Counter(ticks.clone())), interval);
        std::thread::sleep(interval * 8);
        assert!(ticks.load(Ordering::SeqCst) > 1);
        drop(handle);
        std::thread::sleep(interval * 2); // let an in-flight tick land
        let settled = ticks.load(Ordering::SeqCst);
        std::thread::sleep(interval * 6);
        assert_eq!(ticks.load(Ordering::SeqCst), settled);
    }
}
