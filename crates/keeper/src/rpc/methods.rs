use serde::Deserialize;
use serde_json::{Value, json};
use tecpan_wire::messages::EcdhPurpose;

use super::json::{
    JsonCeremony, JsonDkgResult, JsonManifest, JsonMemberPartials, JsonNewMember, JsonParticipant,
    JsonPoint, JsonReshareResult, hex32, parse_bytes, parse32,
};
use crate::{
    Error, Keeper, Result,
    ceremony::dkg::DkgParams,
    keeper::{DkgResult, ReshareRequest, ReshareResult, VaultInfo},
    store::ReceiptEntry,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VaultParam {
    vault: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionParam {
    session: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DkgStart {
    session: String,
    threshold: u32,
    members: Vec<JsonParticipant>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReshareStart {
    session: String,
    vault: String,
    current: Option<JsonManifest>,
    dealers: Vec<u32>,
    new_threshold: u32,
    new_spending_threshold: u32,
    new_members: Vec<JsonNewMember>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Register {
    session: String,
    manifest: JsonManifest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ObserveConfig {
    vault: String,
    version: u32,
    committee_commitment: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ObserveTx {
    vault: String,
    tx_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EcdhBatch {
    vault: String,
    epoch: u32,
    purpose: String,
    points: Vec<JsonPoint>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Body {
    vault: String,
    body: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdParam {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProposalParam {
    proposal: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Receipt {
    vault: String,
    proposal: String,
    tx_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Audit {
    vault: String,
    limit: Option<u32>,
}

fn params<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| Error::Invalid(format!("params: {e}")))
}

fn vault_json(info: &VaultInfo) -> Value {
    json!({
        "manifest": JsonManifest::from(&info.manifest),
        "status": info.status,
        "index": info.index,
        "serving": info.serving,
    })
}

fn receipt_json(receipt: &ReceiptEntry) -> Value {
    json!({
        "proposal": hex32(&receipt.proposal),
        "txHash": hex32(&receipt.tx_hash),
        "at": receipt.at,
    })
}

fn purpose(label: &str) -> Result<EcdhPurpose> {
    match label {
        "decryption" => Ok(EcdhPurpose::NoteDecryption),
        "tagging" => Ok(EcdhPurpose::RecipientTag),
        "sender-tag" => Ok(EcdhPurpose::SenderTag),
        "handshake" => Ok(EcdhPurpose::Handshake),
        other => Err(Error::Invalid(format!("unknown purpose {other}"))),
    }
}

/// Runs one method (everything except `session.auth` and `events.subscribe`, handled by the server).
#[expect(clippy::too_many_lines, reason = "one arm per RPC method")]
pub(super) async fn call(keeper: &Keeper, method: &str, value: Value) -> Result<Value> {
    if method.starts_with("lobby.") {
        return super::lobby::call(keeper, method, value).await;
    }
    match method {
        "keeper.info" => Ok(json!({
            "identity": hex32(&keeper.identity().0),
            "transport": hex32(&keeper.transport_key()),
            "addresses": keeper.addresses(),
            "version": env!("CARGO_PKG_VERSION"),
        })),
        "vault.list" => Ok(Value::Array(
            keeper.vaults().await.iter().map(vault_json).collect(),
        )),
        "vault.status" => {
            let p: VaultParam = params(value)?;
            Ok(vault_json(&keeper.vault(parse32(&p.vault)?).await?))
        }
        "vault.register" => {
            let p: Register = params(value)?;
            Ok(vault_json(
                &keeper
                    .register_vault(parse32(&p.session)?, p.manifest.to_manifest()?)
                    .await?,
            ))
        }
        "vault.observeConfig" => {
            let p: ObserveConfig = params(value)?;
            let observation = keeper
                .observe_config(
                    parse32(&p.vault)?,
                    p.version,
                    parse32(&p.committee_commitment)?,
                )
                .await?;
            Ok(json!({ "observation": observation }))
        }
        "vault.sync" => {
            let p: VaultParam = params(value)?;
            keeper.sync(parse32(&p.vault)?).await?;
            Ok(Value::Bool(true))
        }
        "mailbox.poll" => Ok(json!({ "delivered": keeper.poll_mailbox().await? })),
        "vault.observeTx" => {
            let p: ObserveTx = params(value)?;
            Ok(json!({ "known": keeper.observe_tx(parse32(&p.vault)?, parse32(&p.tx_hash)?)? }))
        }
        "ecdh.batch" => {
            let p: EcdhBatch = params(value)?;
            let points = p
                .points
                .iter()
                .map(JsonPoint::to_wire)
                .collect::<Result<Vec<_>>>()?;
            let responses = keeper
                .ecdh_batch(parse32(&p.vault)?, p.epoch, purpose(&p.purpose)?, points)
                .await?;
            Ok(json!({
                "epoch": p.epoch,
                "responses": responses.iter().map(JsonMemberPartials::from).collect::<Vec<_>>(),
            }))
        }
        "ceremony.dkg" => {
            let p: DkgStart = params(value)?;
            keeper
                .start_dkg(DkgParams {
                    session: parse32(&p.session)?,
                    threshold: p.threshold,
                    members: p
                        .members
                        .iter()
                        .map(JsonParticipant::to_participant)
                        .collect::<Result<_>>()?,
                })
                .await?;
            Ok(Value::Bool(true))
        }
        "ceremony.reshare" => {
            let p: ReshareStart = params(value)?;
            keeper
                .start_reshare(ReshareRequest {
                    session: parse32(&p.session)?,
                    vault: parse32(&p.vault)?,
                    current: p
                        .current
                        .as_ref()
                        .map(JsonManifest::to_manifest)
                        .transpose()?,
                    dealers: p.dealers,
                    new_threshold: p.new_threshold,
                    new_spending_threshold: p.new_spending_threshold,
                    new_members: p
                        .new_members
                        .iter()
                        .map(JsonNewMember::to_new_member)
                        .collect::<Result<_>>()?,
                })
                .await?;
            Ok(Value::Bool(true))
        }
        "ceremony.status" => {
            let p: SessionParam = params(value)?;
            let info = keeper.ceremony(parse32(&p.session)?)?;
            let result = match (info.kind.as_str(), info.result) {
                ("dkg", Some(result)) => serde_json::from_value::<DkgResult>(result)
                    .ok()
                    .map(|r| serde_json::to_value(JsonDkgResult::from(&r)))
                    .transpose()
                    .map_err(|e| Error::Storage(e.to_string()))?,
                ("reshare", Some(result)) => serde_json::from_value::<ReshareResult>(result)
                    .ok()
                    .map(|r| serde_json::to_value(JsonReshareResult::from(&r)))
                    .transpose()
                    .map_err(|e| Error::Storage(e.to_string()))?,
                _ => None,
            };
            serde_json::to_value(JsonCeremony {
                kind: info.kind,
                status: info.status,
                failure: info.failure,
                result,
            })
            .map_err(|e| Error::Storage(e.to_string()))
        }
        "proposals.put" => {
            let p: Body = params(value)?;
            let id = keeper
                .put_proposal(parse32(&p.vault)?, &parse_bytes(&p.body)?)
                .await?;
            Ok(json!({ "id": hex32(&id) }))
        }
        "proposals.list" => {
            let p: VaultParam = params(value)?;
            Ok(json!(
                keeper
                    .proposals(parse32(&p.vault)?)?
                    .iter()
                    .map(|b| format!("0x{}", hex::encode(b)))
                    .collect::<Vec<_>>()
            ))
        }
        "proposals.get" => {
            let p: IdParam = params(value)?;
            Ok(json!(
                keeper
                    .proposal(parse32(&p.id)?)?
                    .map(|b| format!("0x{}", hex::encode(b)))
            ))
        }
        "approvals.put" => {
            let p: Body = params(value)?;
            keeper
                .put_approval(parse32(&p.vault)?, &parse_bytes(&p.body)?)
                .await?;
            Ok(Value::Bool(true))
        }
        "approvals.list" => {
            let p: ProposalParam = params(value)?;
            Ok(json!(
                keeper
                    .approvals(parse32(&p.proposal)?)?
                    .iter()
                    .map(|b| format!("0x{}", hex::encode(b)))
                    .collect::<Vec<_>>()
            ))
        }
        "receipts.put" => {
            let p: Receipt = params(value)?;
            keeper
                .put_receipt(
                    parse32(&p.vault)?,
                    parse32(&p.proposal)?,
                    parse32(&p.tx_hash)?,
                )
                .await?;
            Ok(Value::Bool(true))
        }
        "receipts.list" => {
            let p: VaultParam = params(value)?;
            Ok(Value::Array(
                keeper
                    .receipts(parse32(&p.vault)?)?
                    .iter()
                    .map(receipt_json)
                    .collect(),
            ))
        }
        "audit.list" => {
            let p: Audit = params(value)?;
            serde_json::to_value(
                keeper.audit_log(parse32(&p.vault)?, p.limit.unwrap_or(100).min(10_000))?,
            )
            .map_err(|e| Error::Storage(e.to_string()))
        }
        _ => Err(Error::Invalid(format!("unknown method {method}"))),
    }
}
