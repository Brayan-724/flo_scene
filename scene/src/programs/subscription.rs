use crate::output_sink::*;
use crate::scene_context::*;
use crate::scene_message::*;
use crate::subprogram_id::*;

use futures::prelude::*;

///
/// Sub-programs that can send events should support this 'Subscribe' message (via a filter). This is a request that the
/// program should send its events to the sender of the message.
///
#[derive(Clone, Copy)]
pub struct Subscribe;

impl SceneMessage for Subscribe { }

///
/// Stores the subscribers for an event stream, and forwards events as needed
///
pub struct EventSubscribers<TEventMessage>
where
    TEventMessage: Clone + SceneMessage,
{
    /// The output sinks that will receive the events from this subprogram
    receivers: Vec<(SubProgramId, OutputSink<TEventMessage>)>
}

impl<TEventMessage> EventSubscribers<TEventMessage>
where
    TEventMessage: 'static + Clone + SceneMessage,
{
    ///
    /// Creates a new set of event subscribers
    ///
    pub fn new() -> Self {
        EventSubscribers { 
            receivers: vec![]
        }
    }

    ///
    /// Subscribes a subprogram to the events sent by this object
    ///
    pub fn subscribe(&mut self, program: SubProgramId, context: &SceneContext) {
        // If we can successfully connect to the target, then send events there
        let output_sink = context.send(program);
        let output_sink = if let Ok(output_sink) = output_sink { output_sink } else { return; };

        self.receivers.push((program, output_sink));
    }

    ///
    /// Sends a message to the subscribers to this object
    ///
    /// Returns true if the message is sent to at least one subscriber, or false if there are no subscribers
    ///
    pub async fn send(&mut self, message: TEventMessage) -> bool {
        // Send to all of the streams at once
        let senders = self.receivers.iter_mut()
            .enumerate()
            .map(|(idx, (_, sender))| sender.send(message.clone()).map(move |result| (idx, result)))
            .collect::<Vec<_>>();

        // Wait for all the messages to send
        let mut results = future::join_all(senders).await;

        // Remove any subscribers that generated an error from the subscribers (iterating through the indexes in reverse so we can remove )
        let mut sent_successfully = false;

        results.sort_by(|(a, _), (b, _)| a.cmp(b));

        for (idx, result) in results.into_iter().rev() {
            if result.is_err() {
                // Remove any susbcriber that's no longer attached
                self.receivers.remove(idx);
            } else {
                // At least one message was delivered
                sent_successfully = true;
            }
        }

        // Result is true if we sent at least one event, or false otherwise
        sent_successfully
    }
}
