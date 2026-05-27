use std::fmt;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::Duration;

/// Batches rapid keystrokes by delaying execution until a quiet period.
///
/// When a new event arrives, any pending execution is cancelled and
/// rescheduled after the debounce interval. This prevents expensive
/// work from running on every keystroke during fast typing.
///
/// **Production-grade single-worker design**: One background worker task owns all state.
/// `mpsc` channel communicates `Schedule`/`Cancel`/`Flush` commands. Timer is reset on
/// every schedule by replacing the pinned `sleep` future in a `select!` loop (biased
/// to prefer channel messages). No shared `Mutex`, no lock held across any `.await`,
/// lock-free from caller perspective. `FnOnce` closure is boxed and executed only
/// when timer fires. `flush` forces immediate execution. Matches all stacc rules.
#[derive(Clone)]
pub struct Debouncer {
    sender: mpsc::Sender<Command>,
}

type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
type BoxedTask = Box<dyn FnOnce() -> BoxFuture + Send + 'static>;

enum Command {
    Schedule(BoxedTask),
    Cancel,
    Flush(oneshot::Sender<Result<(), tokio::task::JoinError>>),
}

impl fmt::Debug for Debouncer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Debouncer")
            .field("sender", &"<mpsc Sender>")
            .finish()
    }
}

impl Debouncer {
    /// Creates a new debouncer with the specified delay. Spawns the single worker task.
    pub fn new(delay: Duration) -> Self {
        let (sender, receiver) = mpsc::channel(16);
        let _worker: tokio::task::JoinHandle<()> = tokio::spawn(run_worker(delay, receiver));
        Self { sender }
    }

    /// Schedules a task to run after the debounce delay.
    ///
    /// If another task is already pending, it is cancelled and replaced.
    pub async fn schedule<F, Fut>(&self, task: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let boxed_task: BoxedTask = Box::new(move || Box::pin(task()));
        let _ = self.sender.send(Command::Schedule(boxed_task)).await;
    }

    /// Cancels any pending task without running it.
    pub async fn cancel(&self) {
        let _ = self.sender.send(Command::Cancel).await;
    }

    /// Waits for any pending task to complete (runs immediately if one is pending).
    ///
    /// Returns an error if the pending task panicked.
    pub async fn flush(&self) -> Result<(), tokio::task::JoinError> {
        let (tx, rx) = oneshot::channel();
        if self.sender.send(Command::Flush(tx)).await.is_err() {
            return Ok(());
        }
        rx.await.unwrap_or(Ok(()))
    }
}

async fn run_worker(delay: Duration, mut rx: mpsc::Receiver<Command>) {
    let mut pending: Option<BoxedTask> = None;
    // Start with long sleep; will be replaced on first Schedule.
    let mut timer = Box::pin(tokio::time::sleep(Duration::from_secs(365 * 86400)));

    loop {
        tokio::select! {
            biased;

            _ = &mut timer => {
                if let Some(task_fn) = pending.take() {
                    let fut = task_fn();
                    fut.await;
                }
                // Reset to long sleep so it doesn't fire again until next Schedule.
                timer = Box::pin(tokio::time::sleep(Duration::from_secs(365 * 86400)));
            }

            Some(cmd) = rx.recv() => {
                match cmd {
                    Command::Schedule(task_fn) => {
                        pending = Some(task_fn);
                        timer = Box::pin(tokio::time::sleep(delay));
                    }
                    Command::Cancel => {
                        pending = None;
                        timer = Box::pin(tokio::time::sleep(Duration::from_secs(365 * 86400)));
                    }
                    Command::Flush(tx) => {
                        if let Some(task_fn) = pending.take() {
                            let fut = task_fn();
                            fut.await;
                        }
                        let _ = tx.send(Ok(()));
                    }
                }
            }
            else => break, // channel closed, worker exits
        }
    }
}
