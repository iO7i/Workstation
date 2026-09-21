//! Offline protocol fixture. Built only with runtime-test-fixtures; never shipped.
use serde_json::{json, Value};
use std::{
    io::{self, BufRead, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
fn send(v: Value) {
    let mut out = io::stdout().lock();
    writeln!(out, "{v}").unwrap();
    out.flush().unwrap();
}
fn terminal(status: &str) {
    send(
        json!({"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"fixture-session","turn":{"id":"fixture-turn","status":status,"text":"SECRET_CANARY_SHOULD_NOT_PERSIST"}}}),
    );
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--fixture-verify") {
        println!("SECRET_CANARY_TASK_STDOUT");
        eprintln!("SECRET_CANARY_TASK_STDERR");
        let ok = std::fs::read_to_string("result.txt").ok().as_deref()
            == Some("changed-by-fixture")
            && std::fs::read_to_string("test_contract.txt").ok().as_deref()
                == Some("do-not-modify");
        std::process::exit(if ok { 0 } else { 1 });
    }
    let exe = std::env::current_exe().unwrap();
    let mode = exe.file_stem().unwrap().to_string_lossy().to_string();
    let active = Arc::new(AtomicBool::new(true));
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        let method = v.get("method").and_then(Value::as_str).unwrap_or("");
        let id = v.get("id").cloned().unwrap_or(Value::Null);
        match method {
            "initialize" => {
                if !mode.contains("handshake") {
                    send(json!({"jsonrpc":"2.0","id":id,"result":{}}));
                }
            }
            "initialized" => {}
            "thread/start" | "thread/resume" => {
                if !mode.contains("session-stall") {
                    send(
                        json!({"jsonrpc":"2.0","id":id,"result":{"thread":{"id":"fixture-session"}}}),
                    );
                }
            }
            "thread/read" => {
                assert_eq!(
                    v.pointer("/params/threadId").and_then(Value::as_str),
                    Some("fixture-session")
                );
                send(
                    json!({"jsonrpc":"2.0","id":id,"result":{"thread":{"id":"fixture-session","turns":[{"id":"unrelated","status":"failed"},{"id":"fixture-turn","status":"completed"}]}}}),
                );
            }
            "turn/start" => {
                let count = std::fs::read_to_string("prompt-count.txt")
                    .unwrap()
                    .parse::<u32>()
                    .unwrap();
                std::fs::write("prompt-count.txt", (count + 1).to_string()).unwrap();
                std::fs::write("result.txt", "changed-by-fixture").unwrap();
                if mode.contains("forbidden") {
                    std::fs::write("test_contract.txt", "changed-test").unwrap();
                }
                if mode.contains("early") {
                    terminal("completed");
                }
                send(json!({"jsonrpc":"2.0","id":id,"result":{"turn":{"id":"fixture-turn"}}}));
                if mode.contains("progress") || mode.contains("noise") || mode.contains("duplicate")
                {
                    let flag = active.clone();
                    let noisy = mode.contains("noise");
                    let duplicate = mode.contains("duplicate");
                    std::thread::spawn(move || {
                        for n in 0..10000 {
                            if !flag.load(Ordering::SeqCst) {
                                break;
                            }
                            if noisy {
                                send(
                                    json!({"method":"log/message","params":{"text":"SECRET_CANARY_CHATTER"}}),
                                );
                            } else {
                                send(
                                    json!({"method":"item/started","params":{"threadId":"fixture-session","turnId":"fixture-turn","item":{"id":if duplicate{"same".to_string()}else{n.to_string()}}}}),
                                );
                            }
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    });
                } else if !mode.contains("lost")
                    && !mode.contains("hang")
                    && !mode.contains("early")
                {
                    terminal("completed");
                }
            }
            "turn/interrupt" => {
                active.store(false, Ordering::SeqCst);
                send(json!({"jsonrpc":"2.0","id":id,"result":{}}));
                if !mode.contains("lost") {
                    terminal("interrupted");
                }
            }
            _ => {
                if !id.is_null() {
                    send(
                        json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"fixture method unavailable"}}),
                    );
                }
            }
        }
    }
    active.store(false, Ordering::SeqCst);
}
