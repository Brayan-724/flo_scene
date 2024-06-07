use flo_scene::*;
use flo_scene::programs::*;
use flo_scene_pipe::*;

use futures::prelude::*;
use tokio::io::*;

#[test]
fn error_from_internal_socket() {
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
 
    struct TestSucceeded;
    impl SceneMessage for TestSucceeded { }

    // The command program accepts connections from the socket and interprets the commands
    let command_program = SubProgramId::new();
    scene.add_subprogram(command_program, command_connection_program, 0);

    // The internal socket program lets us stream commands and responses via a socket connection
    let socket_program = SubProgramId::new();
    start_internal_socket_program(&scene, socket_program, parse_command_stream, display_command_responses).unwrap();

    // Socket program is connected to the command program using the command program socket message (which generates connections)
    scene.connect_programs(socket_program, command_program, StreamId::with_message_type::<CommandProgramSocketMessage>()).unwrap();

    // Add another program that talks to the command program via a socket connection
    scene.add_subprogram(SubProgramId::new(), |_input: InputStream<()>, context| async move {
        // Crete a message to send
        let test_commands = "error::message [ \"json\", \"array\" ]\n";

        // Also create an internal buffer to write to
        let (our_side, their_side)          = duplex(1024);
        let (command_input, command_output) = split(their_side);
        let (read_result, write_command)    = split(our_side);

        // Request that the socket program read from the test commands and writes to the internal buffer
        let mut socket_program = context.send(socket_program).unwrap();
        socket_program.send(InternalSocketMessage::CreateInternalSocket(Box::new(command_input), Box::new(command_output))).await.ok().unwrap();

        // Send the test command to the writer
        let mut write_command = write_command;
        println!("> {:?}", test_commands);
        write_command.write_all(&test_commands.bytes().collect::<Vec<u8>>()).await.unwrap();
        write_command.shutdown().await.unwrap();

        // Read the interal buffer to get the final result
        let mut read_result = read_result;
        while let Ok(msg) = read_result.read_u8().await {
            println!("{:?}", msg as char);
        }

        println!("DONE");

        // Indicate successs
        context.send_message(TestSucceeded).await.ok();
    }, 0);

    // Wait for the test program to indicate that it succeeded
    TestBuilder::new()
        .redirect_input(StreamId::with_message_type::<TestSucceeded>())
        .expect_message(|_: TestSucceeded| Ok(()))
        .run_in_scene_with_threads(&scene, test_program, 5);
}