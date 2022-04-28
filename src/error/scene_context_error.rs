use std::thread;

///
/// Errors relating to scene contexts
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum SceneContextError {
    /// The program is not executing in a context where a scene is available
    NoCurrentScene,

    /// The scene was requested from a point where the context was no longer available
    ThreadShuttingDown,
}

impl From<thread::AccessError> for SceneContextError {
    fn from(_err: thread::AccessError) -> SceneContextError {
        SceneContextError::ThreadShuttingDown
    }
}