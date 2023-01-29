use crate::context::*;
use crate::continuation::*;
use crate::error::*;
use crate::message::*;
use crate::releasable::*;
use crate::standard_classes::*;
use crate::value::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use futures::prelude::*;
use futures::channel::mpsc;
use futures::future;

use std::sync::*;

///
/// Creates a sender stream and sends it as a message to the result of the `receive_stream` continuation, returning a stream of the messages sent to
/// that stream, and a continuation to run
///
/// Essentially, the continuation that's passed in should return a block object that takes a single parameter. This function can be used like this:
///
/// ```ignore
/// # // Can't compile this as cargo will say the crate is 'flo_talk' when it's not, which breaks the macro
/// # #[macro_use] extern crate flo_talk_macros;
/// # use flo_talk::*;
/// #[derive(TalkMessageType)]
/// enum HelloWorld { #[message("helloWorld")] Hello, #[message("goodbye")] Goodbye }
///
/// let (mut hello_world_stream, continuation) = talk_stream_from::<HelloWorld>(TalkScript::from("[ :output | output helloWorld. output goodbye. ]"));
/// ```
///
/// The continuation must be run on a runtime before anything can be retrieved from the stream: the function `TalkRuntime::stream_from()` can be used
/// to create a stream that will automatically execute its code in parallel and report any errors.
///
pub fn talk_stream_from<TStreamItem>(receive_sender_stream: impl Into<TalkContinuation<'static>>) -> (impl 'static + Send + Stream<Item=TStreamItem>, TalkContinuation<'static>) 
where
    TStreamItem: 'static + Send + TalkMessageType,
{
    static VALUE_COLON_MSG: Lazy<TalkMessageSignatureId>  = Lazy::new(|| "value:".into());

    // Convert receive_stream into a continuation
    let receive_sender_stream = receive_sender_stream.into();

    // Create the sender stream continuation
    let (sender_stream, receiver) = create_talk_sender::<TStreamItem>();

    // Create a continuation that streams from the receiver channel to the sender channel
    let run_stream = receive_sender_stream.and_then_if_ok(move |receive_sender_stream| {
        sender_stream.and_then_soon_if_ok(move |sender_stream, talk_context| {
            receive_sender_stream.send_message_in_context(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![sender_stream]), talk_context)
        })
    });

    (receiver, run_stream)
}

///
/// Creates a continuation that will process the next value from the source stream for a mapping operation
///
fn talk_map_process_next_value<TStream, TNewItemFuture>(source_stream: TStream, process_value: impl 'static + Send + Fn(TStream::Item, &mut TalkContext) -> TNewItemFuture) -> TalkContinuation<'static>
where
    TStream:        'static + Send + Stream,
    TStream::Item:  'static + Send,
    TNewItemFuture: 'static + Send + Future<Output=Result<(), mpsc::SendError>>,
{
    let mut source_stream = source_stream.boxed();

    // Wait for the stream to return the next value
    TalkContinuation::future_soon(async move {
        let next = source_stream.next().await;

        if let Some(next) = next {
            // Call the processing function
            TalkContinuation::soon(move |talk_context| {
                let future = process_value(next, talk_context);

                // Wait for the future and then read the next value
                TalkContinuation::future_soon(async move {
                    if let Ok(_) = future.await {
                        // Continue if the future indicated all was well
                        talk_map_process_next_value(source_stream, process_value)
                    } else {
                        // Any error just stops the stream
                        ().into()
                    }
                })
            })
        } else {
            // Stop iterating
            ().into()
        }
    })
}

///
/// Maps a stream through a context
///
/// The `process_value` function is called back with the context from the continuation, which is generally best to schedule in the background.
///
pub fn talk_map_stream<TStream, TNewItemType>(source_stream: TStream, process_value: impl 'static + Send + Fn(TStream::Item, &mut TalkContext) -> TNewItemType) -> (impl 'static + Send + Stream<Item=TNewItemType>, TalkContinuation<'static>)
where
    TStream:        'static + Send + Stream,
    TStream::Item:  'static + Send,
    TNewItemType:   'static + Send,
{
    // Create a channel to send the values through
    let (sender, receiver) = mpsc::channel(1);

    // Create a continuation to process the stream in a context
    let sender                  = Arc::new(Mutex::new(sender));
    let process_continuation    = talk_map_process_next_value(source_stream, move |value, talk_context| {
        // Call the processing function to get the mapped value
        let send_value = process_value(value, talk_context);

        // Result is the 'send' future (errors indicate that the target was cancelled and we should stop processing)
        let sender          = Arc::clone(&sender);
        let mut send_value  = Some(send_value);

        // Manually poll the sender (due to lifetime constraints)
        future::poll_fn(move |future_ctxt| {
            use futures::task::{Poll};

            // Take ownership of the sender (really we should have exclusive ownership, but we'd need a way to pass ownership back again)
            let mut sender = sender.lock().unwrap();

            // Poll until the sender is ready to receive a value
            if let Poll::Ready(()) = sender.poll_ready(future_ctxt)? {
                // Send the item once ready
                let send_value = send_value.take().expect("Next value has already been sent");
                sender.start_send(send_value)?;

                Poll::Ready(Ok(()))
            } else {
                // Not ready yet
                Poll::Pending
            }
        })
    });

    // Result is the receiver that supplies the values generated by the continuation
    (receiver, process_continuation)
}

///
/// Creates a continuation that will process the next value from the source stream for a pipe operation
///
fn talk_pipe_process_next_value<TStream>(source_stream: TStream, process_value: impl 'static + Send + Fn(TStream::Item, &mut TalkContext) -> TalkContinuation<'static>) -> TalkContinuation<'static>
where
    TStream:        'static + Send + Stream,
    TStream::Item:  'static + Send,
{
    let mut source_stream = source_stream.boxed();

    // Wait for the stream to return the next value
    TalkContinuation::future_soon(async move {
        let next = source_stream.next().await;

        if let Some(next) = next {
            TalkContinuation::soon(move |talk_context| {
                // Call process_value to get the continuation for this value
                let continuation = process_value(next, talk_context);

                // Evaluate the continuation, then continue with the rest of the stream
                continuation.and_then_soon(move |result, talk_context| {
                    if result.is_error() {
                        // Any error stops the stream
                        ().into()
                    } else {
                        // Process the next value in the stream
                        talk_pipe_process_next_value(source_stream, process_value)
                    }
                })
            })
        } else {
            // Stop iterating
            ().into()
        }
    })
}

///
/// Pipes a stream through a context
///
/// The `process_value` function is called back with the context from the continuation, which is generally best to schedule in the background. Each value is computed by
/// running the continuation and then converting the resulting value (with errors returned in-stream, if any occur). This is useful where a stream requires a lot of processing
/// in the context.
///
/// `talk_map_stream` may be appropriate for cases where less processing in the context is required
///
pub fn talk_pipe_stream<TStream, TNewItemType>(source_stream: TStream, process_value: impl 'static + Send + Fn(TStream::Item, &mut TalkContext) -> TalkContinuation<'static>) -> (impl 'static + Send + Stream<Item=Result<TNewItemType, TalkError>>, TalkContinuation<'static>)
where
    TStream:        'static + Send + Stream,
    TStream::Item:  'static + Send,
    TNewItemType:   'static + Send + TalkValueType,
{
    // Create a channel to send the values through
    let (sender, receiver) = mpsc::channel(1);

    // Create a continuation to process the stream in a context
    let sender                  = Arc::new(Mutex::new(sender));
    let process_continuation    = talk_pipe_process_next_value(source_stream, move |value, talk_context| {
        // The process_value function returns a continuation
        let continuation    = process_value(value, talk_context);
        let sender          = Arc::clone(&sender);

        // Run the continuation to get a result
        continuation.and_then_soon_if_ok(move |value, talk_context| {
            // Convert the value
            let talk_context    = &*talk_context;
            let value           = TalkOwned::new(value, talk_context);
            let mut next_value  = Some(TNewItemType::try_from_talk_value(value, talk_context));

            // Send the value to the result stream
            TalkContinuation::future_soon(async move {
                let poll_result: Result<(), mpsc::SendError> = future::poll_fn(move |future_ctxt| {
                    use futures::task::{Poll};

                    // Take ownership of the sender (really we should have exclusive ownership, but we'd need a way to pass ownership back again)
                    let mut sender = sender.lock().unwrap();

                    // Poll until the sender is ready to receive a value
                    if let Poll::Ready(()) = sender.poll_ready(future_ctxt)? {
                        // Send the item once ready
                        let send_value = next_value.take().expect("Next value has already been sent");
                        sender.start_send(send_value)?;

                        Poll::Ready(Ok(()))
                    } else {
                        // Not ready yet
                        Poll::Pending
                    }
                }).await;

                if let Err(_) = poll_result {
                    TalkError::StreamClosed.into()
                } else {
                    ().into()
                }
            })
        })
    });

    // Result is the receiver that supplies the values generated by the continuation
    (receiver, process_continuation)
}
