use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::Duration;

pub struct TimerFuture {
    shared_state: Arc<Mutex<SharedState>>,
}

/// フューチャーと待機中のスレッド間で共有する状態
struct SharedState {
    /// スリープ時間を消費したかどうか
    completed: bool,

    /// `TimerFuture`が実行されているタスクのウェイカー
    /// スレッドは、`completed = true`に設定した後で、これを使用して`TimerFuture`のタスクを目覚めさせて、
    /// `completed = true`であることを確認して、先に進むようにします。
    waker: Option<Waker>,
}

impl Future for TimerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // タイマーがすでに完了したか確認するために共有された状態を確認する。
        let mut shared_state = self.shared_state.lock().unwrap();
        if shared_state.completed {
            Poll::Ready(())
        } else {
            // タイマーが完了したときに、スレッドが現在のタスクを目覚めさせることができるようにウェイカーを設定して、
            // フューチャーがシドポーリングされて、`completed = true`かを確認することを保証する。
            //
            // 毎回繰り返しウェイカーをクローンするよりも、１回だけ実行したくなる誘惑に駆られる。
            // しかし、`TimerFuture`はエグゼキューター上のタスク間を移動する可能性があり、これよりも古いウェイカーが
            // 誤ったタスクを指してしまい、`TimerFuture`が正しく目覚めない可能性がある。
            //
            // 注意: `Waker::will_wake`関数を使用して、これを確認できますが、ここでは話を単純にするためにそれを省略
            // する。
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl TimerFuture {
    /// 提供されたタイムアウトの後で完了する`TimerFuture`を作成
    pub fn new(duration: Duration) -> Self {
        let shared_state = Arc::new(Mutex::new(SharedState {
            completed: false,
            waker: None,
        }));

        // 新しいスレッドを生成
        let thread_shared_state = Arc::clone(&shared_state);
        thread::spawn(move || {
            thread::sleep(duration);
            let mut shared_state = thread_shared_state.lock().unwrap();
            // タイマーが完了して、ポーリングされたフューチャーの最後のタスクを目覚めさせる信号を送信
            shared_state.completed = true;
            if let Some(waker) = shared_state.waker.take() {
                waker.wake();
            }
        });

        TimerFuture { shared_state }
    }
}
