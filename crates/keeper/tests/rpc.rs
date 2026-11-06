//! Local RPC: authentication, method calls and events over the Unix socket.
#![cfg(unix)]
#![allow(clippy::unwrap_used)]

mod common;

use std::time::Duration;

use common::Harness;
use serde_json::{Value, json};
use tecpan_keeper::{KeeperConfig, rpc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

struct Client {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
    next: u64,
}

impl Client {
    async fn connect(path: &std::path::Path) -> Self {
        let stream = UnixStream::connect(path).await.unwrap();
        let (reader, writer) = stream.into_split();
        Self {
            reader: BufReader::new(reader),
            writer,
            next: 0,
        }
    }

    async fn call(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let request =
            json!({ "jsonrpc": "2.0", "id": self.next, "method": method, "params": params });
        self.writer
            .write_all(format!("{request}\n").as_bytes())
            .await
            .unwrap();
        loop {
            let mut line = String::new();
            tokio::time::timeout(Duration::from_secs(5), self.reader.read_line(&mut line))
                .await
                .unwrap()
                .unwrap();
            let value: Value = serde_json::from_str(&line).unwrap();
            if value.get("id") == Some(&json!(self.next)) {
                return value;
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn sessions_must_authenticate_before_calling() {
    let harness = Harness::new(KeeperConfig::default());
    let keeper = harness.keeper(1);
    let socket = keeper.dir.path().join("k.sock");
    tokio::spawn(rpc::serve(
        keeper.keeper.clone(),
        socket.clone(),
        rpc::RpcToken::new("secret-token"),
    ));
    for _ in 0..100 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    let mut client = Client::connect(&socket).await;
    let refused = client.call("keeper.info", Value::Null).await;
    assert_eq!(refused["error"]["code"], -32_001);
    let wrong = client
        .call("session.auth", json!({ "token": "secret-tokem" }))
        .await;
    assert_eq!(wrong["error"]["code"], -32_001);
    assert_eq!(
        client.call("keeper.info", Value::Null).await["error"]["code"],
        -32_001
    );

    let ok = client
        .call("session.auth", json!({ "token": "secret-token" }))
        .await;
    assert_eq!(ok["result"], true);
    let info = client.call("keeper.info", Value::Null).await;
    assert_eq!(
        info["result"]["identity"],
        format!("0x{}", hex::encode(keeper.keeper.identity().0))
    );
    assert_eq!(
        client.call("vault.list", Value::Null).await["result"],
        json!([])
    );
    assert_eq!(
        client.call("no.such", Value::Null).await["error"]["code"],
        -32_601
    );
    let bad = client
        .call("vault.status", json!({ "vault": "0x12" }))
        .await;
    assert_eq!(bad["error"]["code"], -32_000);
    assert_eq!(
        client.call("events.subscribe", Value::Null).await["result"],
        true
    );
}
