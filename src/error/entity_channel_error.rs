use super::scene_context_error::*;

use futures::channel::mpsc;
use futures::channel::oneshot;

///
/// Errors that can occur when sending to an entity channel
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum EntityChannelError {
    /// The requested entity doesn't exist
    NoSuchEntity,

    /// The entity is no longer listening for these kinds of message
    NotListening,

    /// No scene is available to create the channel
    NoCurrentScene,

    /// The scene was requested from a point where the context was no longer available
    ThreadShuttingDown,
}

impl From<oneshot::Canceled> for EntityChannelError {
    fn from(_: oneshot::Canceled) -> EntityChannelError {
        EntityChannelError::NotListening
    }
}

impl From<mpsc::SendError> for EntityChannelError {
    fn from(_: mpsc::SendError) -> EntityChannelError {
        EntityChannelError::NotListening
    }
}

impl From<SceneContextError> for EntityChannelError {
    fn from(error: SceneContextError) -> EntityChannelError {
        EntityChannelError::from(&error)
    }
}

impl From<&SceneContextError> for EntityChannelError {
    fn from(error: &SceneContextError) -> EntityChannelError {
        match error {
            SceneContextError::NoCurrentScene       => EntityChannelError::NoCurrentScene,
            SceneContextError::ThreadShuttingDown   => EntityChannelError::ThreadShuttingDown,
        }
    }
}
