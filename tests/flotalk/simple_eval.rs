use flo_scene::flotalk::*;

use futures::prelude::*;
use futures::executor;

use std::sync::*;

#[test]
fn evaluate_number() {
    let test_source     = "42";
    let runtime         = TalkRuntime::empty();
    let root_values     = vec![Arc::new(Mutex::new(TalkValueStore::default()))];

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_continuation(talk_evaluate_simple(root_values, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn add_numbers() {
    let test_source     = "38 + 4";
    let runtime         = TalkRuntime::empty();
    let root_values     = vec![Arc::new(Mutex::new(TalkValueStore::default()))];

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_continuation(talk_evaluate_simple(root_values, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn divide_numbers() {
    let test_source     = "1021.2 // 24.2";
    let runtime         = TalkRuntime::empty();
    let root_values     = vec![Arc::new(Mutex::new(TalkValueStore::default()))];

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_continuation(talk_evaluate_simple(root_values, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn and_success() {
    let test_source     = "(1 < 2) and: [ (3 < 4) ]";
    let runtime         = TalkRuntime::empty();
    let root_values     = vec![Arc::new(Mutex::new(TalkValueStore::default()))];

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_continuation(talk_evaluate_simple(root_values, Arc::new(instructions))).await;

        assert!(result == TalkValue::Bool(true));
    });
}

#[test]
fn and_failure_rhs() {
    let test_source     = "(1 < 2) and: [ (3 > 4) ]";
    let runtime         = TalkRuntime::empty();
    let root_values     = vec![Arc::new(Mutex::new(TalkValueStore::default()))];

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_continuation(talk_evaluate_simple(root_values, Arc::new(instructions))).await;

        assert!(result == TalkValue::Bool(false));
    });
}

#[test]
fn and_failure_lhs() {
    let test_source     = "(1 > 2) and: [ (3 < 4) ]";
    let runtime         = TalkRuntime::empty();
    let root_values     = vec![Arc::new(Mutex::new(TalkValueStore::default()))];

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_continuation(talk_evaluate_simple(root_values, Arc::new(instructions))).await;

        assert!(result == TalkValue::Bool(false));
    });
}

#[test]
fn retrieve_argument() {
    let test_source     = "x";
    let runtime         = TalkRuntime::empty();
    let root_values     = vec![Arc::new(Mutex::new(TalkValueStore::default()))];
    let mut arguments   = TalkValueStore::default();

    arguments.set_symbol_value("x", TalkValue::Int(42));

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_continuation(talk_evaluate_simple_with_arguments(root_values, arguments, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn retrieve_root_value() {
    let test_source     = "x";
    let runtime         = TalkRuntime::empty();
    let root_values     = vec![Arc::new(Mutex::new(TalkValueStore::default()))];

    root_values[0].lock().unwrap().set_symbol_value("x", TalkValue::Int(42));

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_continuation(talk_evaluate_simple(root_values, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn call_block() {
    let test_source     = "[ 42 ] value";
    let runtime         = TalkRuntime::empty();
    let root_values     = vec![Arc::new(Mutex::new(TalkValueStore::default()))];

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_continuation(talk_evaluate_simple(root_values, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn call_block_with_arguments() {
    let test_source     = "[ :x | ^x ] value: 42";
    let runtime         = TalkRuntime::empty();
    let root_values     = vec![Arc::new(Mutex::new(TalkValueStore::default()))];

    executor::block_on(async { 
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_continuation(talk_evaluate_simple(root_values, Arc::new(instructions))).await;

        assert!(result == TalkValue::Int(42));
    });
}
