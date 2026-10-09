//! Regression coverage for cancellation while a provider produces no bytes.

use cortex_core::CortexError;
use cortex_runtime::model::{ModelOutput, ModelProvider};
use cortex_runtime::providers::{AnthropicProvider, OpenAiCompatibleProvider};
use cortex_runtime::{AgentContext, CancellationToken};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
enum Stall {
    Headers,
    JsonBody,
    PartialStreamLine,
}

fn assert_stalled_request_cancels(anthropic: bool, streaming: bool, stall: Stall) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (ready_tx, ready_rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            socket.read_exact(&mut byte).unwrap();
            headers.push(byte[0]);
        }
        let length: usize = String::from_utf8(headers)
            .unwrap()
            .lines()
            .find_map(|line| {
                line.to_lowercase()
                    .strip_prefix("content-length: ")
                    .map(str::to_owned)
            })
            .unwrap()
            .parse()
            .unwrap();
        socket.read_exact(&mut vec![0; length]).unwrap();
        match stall {
            Stall::Headers => {}
            Stall::JsonBody => socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 500\r\n\r\n{").unwrap(),
            Stall::PartialStreamLine => socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"choices\":").unwrap(),
        }
        ready_tx.send(()).unwrap();
        let mut byte = [0];
        // Cancellation must close the client connection, not leave a detached
        // worker waiting for the normal provider timeout.
        match socket.read(&mut byte) {
            Ok(0) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
                ) => {}
            other => panic!("connection survived cancellation: {other:?}"),
        }
    });
    let token = CancellationToken::new();
    let canceller = token.clone();
    let cancel = std::thread::spawn(move || {
        ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let at = Instant::now();
        canceller.cancel();
        at
    });
    let context = AgentContext::new("cancel stalled request").with_cancellation_token(token);
    let provider: Box<dyn ModelProvider> = if anthropic {
        Box::new(AnthropicProvider::new(
            "claude-test",
            Some("test-key".into()),
            Some(url),
        ))
    } else {
        Box::new(OpenAiCompatibleProvider::new("local-test", None, Some(url)))
    };
    let result = if streaming {
        provider.stream(&context, &mut |_| Ok(()))
    } else {
        provider.generate(&context)
    };
    let cancelled_at = cancel.join().unwrap();
    assert!(
        matches!(result, Err(CortexError::Cancelled(_))),
        "{result:?}"
    );
    assert!(cancelled_at.elapsed() < Duration::from_secs(2));
    server.join().unwrap();
}

#[test]
fn openai_stream_cancels_before_headers() {
    assert_stalled_request_cancels(false, true, Stall::Headers);
}

#[test]
fn openai_stream_cancels_during_partial_line() {
    assert_stalled_request_cancels(false, true, Stall::PartialStreamLine);
}

#[test]
fn openai_generate_cancels_during_body() {
    assert_stalled_request_cancels(false, false, Stall::JsonBody);
}

#[test]
fn anthropic_cancels_before_headers() {
    assert_stalled_request_cancels(true, false, Stall::Headers);
}

#[test]
fn anthropic_default_stream_cancels_during_body() {
    assert_stalled_request_cancels(true, true, Stall::JsonBody);
}

#[test]
fn streaming_preserves_fragmented_utf8_and_stops_at_done() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            socket.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        let length: usize = String::from_utf8(request)
            .unwrap()
            .lines()
            .find_map(|line| {
                line.to_lowercase()
                    .strip_prefix("content-length: ")
                    .map(str::to_owned)
            })
            .unwrap()
            .parse()
            .unwrap();
        socket.read_exact(&mut vec![0; length]).unwrap();
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
        for byte in
            "data: {\"choices\":[{\"delta\":{\"content\":\"héllo\"}}]}\r\n\r\ndata: [DONE]\n\n"
                .as_bytes()
        {
            socket.write_all(&[*byte]).unwrap();
        }
    });
    let provider = OpenAiCompatibleProvider::new("local-test", None, Some(url));
    let mut tokens = String::new();
    let output = provider
        .stream(&AgentContext::new("test"), &mut |s| {
            tokens.push_str(s);
            Ok(())
        })
        .unwrap();
    assert_eq!(tokens, "héllo");
    assert_eq!(output, ModelOutput::FinalAnswer("héllo".into()));
    server.join().unwrap();
}

#[test]
fn cancellation_on_final_token_is_persisted_instead_of_completed() {
    use cortex_runtime::model::MockModelProvider;
    use cortex_runtime::storage::RunStore;
    use cortex_runtime::tool::ToolRegistry;
    use cortex_runtime::AgentLoop;
    use std::sync::Arc;

    let store = Arc::new(RunStore::in_memory().unwrap());
    let token = CancellationToken::new();
    let callback_token = token.clone();
    let provider = MockModelProvider::new();
    provider.queue_response(ModelOutput::FinalAnswer("final-token".into()));
    let mut context = AgentContext::new("cancel final token");
    let agent = AgentLoop::new(1)
        .with_store(store.clone())
        .with_cancellation_token(token)
        .with_token_callback(Arc::new(move |_| callback_token.cancel()));
    let result = agent.run(&mut context, &provider, &ToolRegistry::new());
    assert!(matches!(result, Err(CortexError::Cancelled(_))));
    assert_eq!(
        store.get_run(&context.run_id).unwrap().unwrap().status,
        "cancelled"
    );
    let events = store.get_events(&context.run_id).unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event.event_type() == "RunCancelled")
            .count(),
        1
    );
    assert!(!events
        .iter()
        .any(|event| event.event.event_type() == "RunCompleted"));
}
