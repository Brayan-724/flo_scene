use flo_scene::flotalk::*;

use futures::prelude::*;
use futures::executor;

use std::sync::*;

#[test]
fn create_subclass() {
    let test_source     = "Object subclass";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        // Manually create the 'object' in this context (by sending 'new' to the script class class)
        let object = runtime.run_continuation(TalkContinuation::soon(|talk_context| {
            SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
        })).await;

        // Run the test script with the 'Object' class defined
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![("Object".into(), object.clone())], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        // Must generate a new class, using the SCRIPT_CLASS_CLASS
        assert!(result != object);
        assert!(match result {
            TalkValue::Reference(new_class) => new_class.class() == *SCRIPT_CLASS_CLASS,
            _ => false
        });
    });
}

#[test]
fn read_superclass() {
    let test_source     = "(Object subclass) superclass";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        // Manually create the 'object' in this context (by sending 'new' to the script class class)
        let object = runtime.run_continuation(TalkContinuation::soon(|talk_context| {
            SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
        })).await;

        // Run the test script with the 'Object' class defined
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        //let result          = runtime.run_with_symbols(|_| vec![("Object".into(), object.clone())], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        // Superclass gets us back to 'object'
        //assert!(result == object);
    });
}
