use std::future::Future;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::task::Context;
use std::time::Duration;

use futures::future::{BoxFuture, FutureExt};
use futures::task::{waker_ref, ArcWake};

use timer_future::TimerFuture;

fn main() {
    let (executor, spawner) = new_executor_and_spawner();

    // タイマーを待っている前後で印字するタスクを生成する。
    spawner.spawn(async {
        println!("howdy!");
        // 2秒後に完了するタイマー・フューチャーを待つ。
        TimerFuture::new(Duration::new(2, 0)).await;
        println!("done!");
    });

    // エグゼキューターは、それが終了して、実行する入力タスクをうことらないことを認識しているため、
    // 生成者をドロップする。
    drop(spawner);

    // タスク・キューが空になるまで、エグゼキューターを実行する。
    // これは"howdy!"と印字、停止、その後"done!"と印字する。
    executor.run();
}

/// チャネルからタスクを受け取り、それらを実行するタスク・エグゼキューター
struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

/// `Spawner`はタスク・チャネル上でフューチャーを生み出す
#[derive(Clone)]
struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

/// `Executor`によってポーリングされるように、自身で再スケジュールするフューチャー
struct Task {
    /// 完了するまで進められる進行中のフューチャー
    ///
    /// 1度にタスクを実行する1つのスレッドしかないため、正確には`Mutex`は必要ない。
    /// しかし、Rustは、`future`が1つのスレッドのみ変更されることを認識するほど十分に賢くないので、
    /// スレッドの安全性を証明するために`Mutex`を使用する必要があります。
    /// プロダクションのエグゼキューターはこれを必要とせず、代わりに`UnsafeCell`を使用できます。
    future: Mutex<Option<BoxFuture<'static, ()>>>,

    /// タスク自身でタスク・キューに戻すハンドル
    task_sender: SyncSender<Arc<Task>>,
}

fn new_executor_and_spawner() -> (Executor, Spawner) {
    // 一度にチャネル内にキューイングできる最大のタスク数
    // これは単に`sync_channel`を満足させるものであり、実際のエグゼキューターには存在しない。
    const MAX_QUEUED_TASKS: usize = 10_000;

    let (task_sender, ready_queue) = sync_channel(MAX_QUEUED_TASKS);

    (Executor { ready_queue }, Spawner { task_sender })
}

impl Spawner {
    fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        let future = future.boxed();
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });
        self.task_sender.send(task).expect("too many tasks queued");
    }
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // タスクがエグゼキューターによって再度ポーリングされるようにするため、
        // このタスクをタスク・チャネルに戻すことによって`wake`を実装する。
        let cloned = Arc::clone(arc_self);
        arc_self
            .task_sender
            .send(cloned)
            .expect("too many tasks queued");
    }
}

impl Executor {
    fn run(&self) {
        while let Ok(task) = self.ready_queue.recv() {
            // フューチャーを受け取り、もし、それがまだ完了していない場合（まだある）、
            // 完了するためにそれをポーリングする。
            let mut future_slot = task.future.lock().unwrap();
            if let Some(mut future) = future_slot.take() {
                // タスク自身から`LocalWaker`を作成する。
                let waker = waker_ref(&task);
                let context = &mut Context::from_waker(&waker);
                // `BoxFuture<T>`は、`Pin<Box<dyn Future<Output = T> + Send +'static>>`の
                // 型エイリアスである。
                // `Pin::as_mut`メソッドを呼び出すことで、それから`Pin<&mut dyn Future + Send + 'static>`
                // を得ることができる。
                if future.as_mut().poll(context).is_pending() {
                    // フューチャーの処理を終了できなかったので、将来、再度処理されるように、それをそのタスクに押し込む。
                    *future_slot = Some(future);
                }
            }
        }
    }
}
