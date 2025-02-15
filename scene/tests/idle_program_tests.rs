//!
//! The idle request program is used to notify when a scene has become idle, which is to say
//! that it has processed all of the messages that have been sent and is waiting for new ones
//!

use flo_scene::*;
use flo_scene::programs::*;

use futures::future;

#[test]
fn notify_on_idle() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();

    TestBuilder::new()
        .send_message(IdleRequest::WhenIdle(test_program))
        .expect_message(|IdleNotification| { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn notifies_if_subprogram_drops_input_stream() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();

    // This will drop the input stream before processing any messages, will happen a lot when we create subprograms that don't process any input
    scene.add_subprogram(SubProgramId::new(), |_: InputStream<()>, _| async move {
        future::pending::<()>().await;
    }, 0);

    TestBuilder::new()
        .send_message(IdleRequest::WhenIdle(test_program))
        .expect_message(|IdleNotification| { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}
