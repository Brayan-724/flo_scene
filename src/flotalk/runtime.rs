use super::context::*;
use super::continuation::*;
use super::error::*;
use super::value::*;

use futures::prelude::*;
use futures::future;
use futures::lock;
use futures::task::{Poll, Context};

use std::sync::*;

// TODO: write and upgrade to a 'fair' mutex that processing wakeups in the order that they happen

///
/// A `TalkRuntime` is used to run continuations inside a `TalkContext` (it wraps a TalkContext,
/// and schedules continuations on them)
///
pub struct TalkRuntime {
    pub (crate) context: Arc<lock::Mutex<TalkContext>>
}

impl TalkRuntime {
    ///
    /// Creates a runtime for a context
    ///
    pub fn with_context(context: TalkContext) -> TalkRuntime {
        TalkRuntime {
            context: Arc::new(lock::Mutex::new(context))
        }
    }

    ///
    /// Returns an empty runtime
    ///
    pub fn empty() -> TalkRuntime {
        Self::with_context(TalkContext::empty())
    }

    ///
    /// Runs a continuation with a 'later' part
    ///
    fn run_continuation_later(&self, later: Box<dyn Send + FnMut(&mut TalkContext, &mut Context) -> Poll<TalkValue>>) -> impl Send + Future<Output=TalkValue> {
        // If the runtime is dropped while the future is running, it will be aborted (if it ever wakes up again)
        let talk_context        = Arc::downgrade(&self.context);
        let mut acquire_context = None;
        let mut later           = later;

        // Poll the 'later' whenever the context is available
        future::poll_fn(move |future_context| {
            if let Some(talk_context) = talk_context.upgrade() {
                // Often we can just acquire the mutex immediately
                if acquire_context.is_none() {
                    // Don't try_lock() if we're acquiring the context via the mutex
                    if let Some(mut talk_context) = talk_context.try_lock() {
                        acquire_context = None;
                        return later(&mut *talk_context, future_context);
                    }
                }

                // Start locking the context if it's currently released
                if acquire_context.is_none() {
                    acquire_context = Some(lock::Mutex::lock_owned(talk_context));
                }

                if let Poll::Ready(mut talk_context) = acquire_context.as_mut().unwrap().poll_unpin(future_context) {
                    // Acquired access to the context
                    acquire_context = None;

                    return later(&mut *talk_context, future_context);
                } else {
                    // Context is in use on another thread
                    return Poll::Pending;
                }
            } else {
                // Context is not available
                acquire_context = None;

                Poll::Ready(TalkValue::Error(TalkError::RuntimeDropped))
            }
        })
    }

    ///
    /// Runs a continuation on this runtime
    ///
    #[inline]
    pub fn run_continuation(&self, continuation: TalkContinuation) -> impl Send + Future<Output=TalkValue> {
        enum NowLater<T> {
            Now(TalkValue),
            Later(T),
        }

        let now_later = match continuation {
            TalkContinuation::Ready(value)  => NowLater::Now(value),
            TalkContinuation::Later(later)  => NowLater::Later(self.run_continuation_later(later)),

            TalkContinuation::Soon(soon)    => {
                let mut soon = Some(soon);

                NowLater::Later(self.run_continuation_later(Box::new(move |talk_context, _| Poll::Ready((soon.take().unwrap())(talk_context)))))
            },
        };

        async move {
            match now_later {
                NowLater::Now(value)    => value,
                NowLater::Later(later)  => later.await,
            }
        }
    }
}
