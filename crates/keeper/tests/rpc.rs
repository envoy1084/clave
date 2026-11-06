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

async fn authenticated(keeper: &common::TestKeeper) -> Client {
    let socket = keeper.dir.path().join("k.sock");
    tokio::spawn(rpc::serve(
        keeper.keeper.clone(),
        socket.clone(),
        rpc::RpcToken::new("token"),
    ));
    for _ in 0..100 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let mut client = Client::connect(&socket).await;
    assert_eq!(
        client
            .call("session.auth", json!({ "token": "token" }))
            .await["result"],
        true
    );
    client
}

fn json_point(seed: u64) -> Value {
    let point = common::point(seed);
    json!({
        "x": format!("0x{}", hex::encode(point.x)),
        "y": format!("0x{}", hex::encode(point.y)),
    })
}

/// Calls `method` and returns its result, printing the exchange (`--nocapture` shows the JSON shapes).
async fn exchange(client: &mut Client, method: &str, params: Value) -> Value {
    let response = client.call(method, params.clone()).await;
    eprintln!("--> {method} {params}\n<-- {response}");
    assert!(response.get("error").is_none(), "{method}: {response}");
    response["result"].clone()
}

async fn lobby_status(client: &mut Client, lobby: &str) -> Value {
    client.call("lobby.status", json!({ "lobby": lobby })).await["result"].clone()
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines)]
async fn lobby_methods_found_a_vault_plan() {
    let harness = Harness::new(KeeperConfig::default());
    let host = harness.keeper(1);
    let guest = harness.keeper(2);
    let mut a = authenticated(&host).await;
    let mut b = authenticated(&guest).await;
    // Events go to their own session: `call` skips notifications while it waits for its answer.
    let mut events = Client::connect(&guest.dir.path().join("k.sock")).await;
    events
        .call("session.auth", json!({ "token": "token" }))
        .await;
    assert_eq!(
        events.call("events.subscribe", Value::Null).await["result"],
        true
    );

    let opened = exchange(
        &mut a,
        "lobby.open",
        json!({
            "name": "Treasury",
            "threshold": 2,
            "viewingThreshold": 2,
            "network": "local-network",
            "member": { "name": "Ada", "signerKey": json_point(1) },
        }),
    )
    .await;
    let lobby = opened["lobby"].as_str().unwrap().to_owned();
    let invite = opened["invite"].as_str().unwrap().to_owned();
    assert!(invite.starts_with("tecpan-invite-v0:"));

    let preview = exchange(&mut b, "lobby.inspect", json!({ "invite": invite })).await;
    assert_eq!(preview["lobby"], lobby);
    assert_eq!(preview["hostName"], "Ada");
    assert_eq!(preview["viewingThreshold"], 2);
    assert_eq!(
        preview["host"]["identity"],
        format!("0x{}", hex::encode(host.keeper.identity().0))
    );
    assert_eq!(
        b.call(
            "lobby.inspect",
            json!({ "invite": "tecpan-invite-v0:AAAA" })
        )
        .await["error"]["code"],
        -32_000
    );
    assert_eq!(b.call("lobby.list", Value::Null).await["result"], json!([]));

    let joined = exchange(
        &mut b,
        "lobby.join",
        json!({ "invite": invite, "member": { "name": "Grace", "signerKey": json_point(2) } }),
    )
    .await;
    assert_eq!(joined["lobby"], lobby);
    for client in [&mut a, &mut b] {
        for _ in 0..500 {
            if lobby_status(client, &lobby).await["members"]
                .as_array()
                .map(Vec::len)
                == Some(2)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    let host_view = exchange(&mut a, "lobby.status", json!({ "lobby": lobby })).await;
    assert_eq!(host_view["role"], "host");
    assert_eq!(host_view["status"], "open");
    assert_eq!(host_view["invite"], invite);
    assert_eq!(host_view["members"][0]["you"], true);
    assert_eq!(host_view["members"][1]["name"], "Grace");
    assert_eq!(host_view["members"][1]["index"], 2);
    let guest_view = exchange(&mut b, "lobby.status", json!({ "lobby": lobby })).await;
    assert_eq!(guest_view["role"], "guest");
    assert_eq!(guest_view["members"][1]["you"], true);
    assert!(guest_view.get("invite").is_none());

    let secrets = format!("0x{}", hex::encode(b"protocol secrets"));
    let salt = format!("0x{}", hex::encode([7u8; 32]));
    let started = exchange(
        &mut a,
        "lobby.start",
        json!({
            "lobby": lobby, "threshold": 2, "viewingThreshold": 2, "salt": salt, "secrets": secrets,
        }),
    )
    .await;
    assert_eq!(started["status"], "started");
    assert_eq!(started["plan"]["secrets"], secrets);
    let session = started["plan"]["session"].clone();
    let mut received = Value::Null;
    for _ in 0..500 {
        received = lobby_status(&mut b, &lobby).await;
        if received["status"] == "started" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    eprintln!("<-- lobby.status (guest, started) {received}");
    assert_eq!(received["plan"]["session"], session);
    assert_eq!(received["plan"]["salt"], salt);
    assert_eq!(received["plan"]["secrets"], secrets);
    let listed = exchange(&mut b, "lobby.list", Value::Null).await;
    assert!(listed[0]["plan"].get("secrets").is_none());
    assert_eq!(listed[0]["plan"]["session"], session);

    // The guest's event stream carried the lobby's progress.
    let mut seen = Vec::new();
    while !seen.iter().any(|s| s == "started") {
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(5), events.reader.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        let value: Value = serde_json::from_str(&line).unwrap();
        if value["method"] == "event" && value["params"]["type"] == "lobby" {
            eprintln!("<-- event {value}");
            assert_eq!(value["params"]["lobby"], lobby.trim_start_matches("0x"));
            seen.push(value["params"]["status"].as_str().unwrap().to_owned());
        }
    }

    let refused = a.call("lobby.close", json!({ "lobby": lobby })).await;
    assert_eq!(
        refused["error"]["code"], -32_000,
        "a started lobby cannot be closed"
    );
}
