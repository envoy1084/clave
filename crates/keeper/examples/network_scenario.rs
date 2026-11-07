//! Drives keepers through their RPC sockets: DKG, vault registration, threshold ECDH from two members (the combined
//! points must agree) and proposal sync. Used by the network-namespace test (`tests/network/run.sh`), where each
//! keeper sits in its own network and reaches the others only through a relay.
//!
//! `network_scenario <keeper data dir>...` (each dir holds `keeper.sock` and `rpc.token`).
#![allow(
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines
)]

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use clave_crypto::{
    Base, Point,
    aztec::AddressContext,
    curve,
    ecdh::{DleqProof, Partial, combine},
    shamir::random_scalar,
};
use serde_json::{Value, json};
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
    async fn connect(dir: &Path) -> Self {
        let deadline = Instant::now() + Duration::from_mins(2);
        loop {
            if let (Ok(stream), Ok(token)) = (
                UnixStream::connect(dir.join("keeper.sock")).await,
                std::fs::read_to_string(dir.join("rpc.token")),
            ) {
                let (reader, writer) = stream.into_split();
                let mut client = Self {
                    reader: BufReader::new(reader),
                    writer,
                    next: 0,
                };
                client
                    .call("session.auth", json!({ "token": token.trim() }))
                    .await;
                return client;
            }
            assert!(
                Instant::now() < deadline,
                "keeper at {} never came up",
                dir.display()
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
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
            self.reader.read_line(&mut line).await.unwrap();
            let reply: Value = serde_json::from_str(&line).unwrap();
            if reply["id"] == json!(self.next) {
                assert!(reply.get("error").is_none(), "{method}: {}", reply["error"]);
                return reply["result"].clone();
            }
        }
    }
}

fn hex32(bytes: &[u8; 32]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn bytes32(value: &Value) -> [u8; 32] {
    let mut out = [0u8; 32];
    hex::decode_to_slice(value.as_str().unwrap().trim_start_matches("0x"), &mut out).unwrap();
    out
}

fn point(value: &Value) -> Point {
    Point::from_xy(
        curve::base_from_bytes(&bytes32(&value["x"])).unwrap(),
        curve::base_from_bytes(&bytes32(&value["y"])).unwrap(),
    )
    .unwrap()
}

fn json_point(p: &Point) -> Value {
    json!({ "x": hex32(&curve::base_to_bytes(&p.x())), "y": hex32(&curve::base_to_bytes(&p.y())) })
}

fn random32() -> [u8; 32] {
    let mut out = [0u8; 32];
    getrandom::fill(&mut out).unwrap();
    out
}

async fn until(client: &mut Client, session: &str) -> Value {
    let deadline = Instant::now() + Duration::from_mins(3);
    loop {
        let status = client
            .call("ceremony.status", json!({ "session": session }))
            .await;
        match status["status"].as_str().unwrap() {
            "completed" => return status["result"].clone(),
            "aborted" => panic!("ceremony aborted: {}", status["failure"]),
            _ => {}
        }
        assert!(Instant::now() < deadline, "ceremony did not finish");
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// `address_secret · P` combined from one keeper's ECDH batch.
async fn ecdh(client: &mut Client, vault: &str, input: &Point) -> Point {
    let answer = client
        .call("ecdh.batch", json!({ "vault": vault, "epoch": 0, "purpose": "decryption", "points": [json_point(input)] }))
        .await;
    let partials: Vec<Partial> = answer["responses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let p = &r["partials"][0];
            Partial {
                index: u32::try_from(r["member"].as_u64().unwrap()).unwrap(),
                point: point(&p["point"]),
                proof: DleqProof {
                    challenge: curve::scalar_from_bytes(&bytes32(&p["challenge"])).unwrap(),
                    response: curve::scalar_from_bytes(&bytes32(&p["response"])).unwrap(),
                },
            }
        })
        .collect();
    combine(&partials, partials.len()).unwrap()
}

#[tokio::main]
async fn main() {
    let dirs: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    assert!(
        dirs.len() >= 2,
        "usage: network_scenario <keeper data dir>..."
    );
    let started = Instant::now();
    let mut clients = Vec::new();
    for dir in &dirs {
        clients.push(Client::connect(dir).await);
    }
    let mut members = Vec::new();
    for client in &mut clients {
        let info = client.call("keeper.info", Value::Null).await;
        println!("keeper {} at {}", info["identity"], info["addresses"]);
        members.push(json!({ "identity": info["identity"], "transport": info["transport"], "addresses": info["addresses"] }));
    }

    // DKG among all keepers.
    let session = hex32(&random32());
    let dkg_started = Instant::now();
    for client in &mut clients {
        client
            .call(
                "ceremony.dkg",
                json!({ "session": session, "threshold": 2, "members": members }),
            )
            .await;
    }
    let mut results = Vec::new();
    for client in &mut clients {
        results.push(until(client, &session).await);
    }
    assert!(
        results
            .iter()
            .all(|r| r["transcript"] == results[0]["transcript"]),
        "transcripts differ"
    );
    let dkg_ms = dkg_started.elapsed().as_millis();
    println!("dkg completed in {dkg_ms} ms");

    // Register the vault everywhere.
    let mut rng = rand_chacha_rng();
    let preaddress =
        curve::base_from_bytes(&curve::scalar_to_bytes(random_scalar(&mut rng).expose()))
            .unwrap_or(Base::from(7u64));
    let group_key = point(&results[0]["groupKey"]);
    let vault = hex32(&curve::base_to_bytes(
        &AddressContext::new(preaddress, &group_key)
            .unwrap()
            .address(),
    ));
    let manifest_members: Vec<Value> = members
        .iter()
        .zip(results[0]["verificationShares"].as_array().unwrap())
        .zip(1..)
        .map(|((m, share), index)| {
            let signer = Point::generator_mul_secret(&random_scalar(&mut rng)).unwrap();
            json!({
                "index": index, "signerKey": json_point(&signer), "verificationShare": share,
                "identity": m["identity"], "transport": m["transport"], "addresses": m["addresses"],
            })
        })
        .collect();
    let manifest = json!({
        "vault": vault, "epoch": 0, "threshold": 2, "viewingThreshold": 2,
        "preaddress": hex32(&curve::base_to_bytes(&preaddress)), "groupKey": results[0]["groupKey"], "members": manifest_members,
    });
    for client in &mut clients {
        client
            .call(
                "vault.register",
                json!({ "session": session, "manifest": manifest }),
            )
            .await;
    }

    // Threshold ECDH served to two different members' engines.
    let input = Point::generator_mul_secret(&random_scalar(&mut rng)).unwrap();
    let ecdh_started = Instant::now();
    let first = ecdh(&mut clients[0], &vault, &input).await;
    let ecdh_ms = ecdh_started.elapsed().as_millis();
    let last = clients.len() - 1;
    let second = ecdh(&mut clients[last], &vault, &input).await;
    assert_eq!(first, second, "members disagree on address_secret · P");
    println!("ecdh agreed ({ecdh_ms} ms for the first batch)");

    // Proposal sync.
    let proposal = clave_wire::encode_body(&clave_wire::Message::Proposal(
        clave_wire::messages::Proposal {
            config_version: 0,
            outer_hash: [3; 32],
            expires_at: 1_900_000_000,
            tx_nonce: random32(),
            fee_method: 0,
            payload: vec![1, 2, 3],
            summary: "network test".into(),
            proposer: 1,
        },
    ))
    .unwrap();
    let body = format!("0x{}", hex::encode(&proposal));
    clients[1]
        .call("proposals.put", json!({ "vault": vault, "body": body }))
        .await;
    for (i, client) in clients.iter_mut().enumerate() {
        let deadline = Instant::now() + Duration::from_mins(1);
        loop {
            let held = client
                .call("proposals.list", json!({ "vault": vault }))
                .await;
            if held
                .as_array()
                .unwrap()
                .iter()
                .any(|b| b.as_str() == Some(body.as_str()))
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "keeper {i} never received the proposal"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
    println!("proposal synced to every keeper");
    println!(
        "{}",
        json!({ "result": "PASS", "keepers": clients.len(), "dkgMs": dkg_ms, "ecdhMs": ecdh_ms, "totalMs": started.elapsed().as_millis() })
    );
}

fn rand_chacha_rng() -> rand_chacha::ChaCha20Rng {
    rand_core::SeedableRng::from_seed(random32())
}
