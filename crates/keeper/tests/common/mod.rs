//! A multi-keeper harness on the in-memory network with a manual clock.
#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::missing_panics_doc
)]

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use tecpan_crypto::{
    Base, Point, SecretScalar,
    aztec::AddressContext,
    ecdh::{Partial, combine},
    shamir::random_scalar,
};
use tecpan_keeper::{
    Clock, DkgResult, Keeper, KeeperConfig, KeeperParts,
    ceremony::{Participant, dkg::DkgParams},
    custody::{KeeperKeys, MasterKey, SecretStore},
    mailbox::{MailboxService, MemoryMailbox},
    store::Store,
    transport::MemoryNetwork,
};
use tecpan_wire::{
    messages::{EcdhPurpose, Manifest, Member},
    types::{WirePoint, field},
};
use tempfile::TempDir;

/// A clock tests move by hand.
#[derive(Default)]
pub struct ManualClock(AtomicU64);

impl ManualClock {
    pub fn advance(&self, seconds: u64) {
        self.0.fetch_add(seconds, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> u64 {
        1_700_000_000 + self.0.load(Ordering::SeqCst)
    }
}

pub struct TestKeeper {
    pub keeper: Keeper,
    pub dir: TempDir,
    pub master: [u8; 32],
}

impl TestKeeper {
    pub fn participant(&self) -> Participant {
        Participant {
            identity: self.keeper.identity(),
            transport: self.keeper.transport_key(),
            addresses: Vec::new(),
        }
    }

    /// Re-reads this keeper's long-lived keys from its sealed store (for forging its messages in tests).
    pub fn keys(&self) -> KeeperKeys {
        let secrets = SecretStore::open(
            self.dir.path().join("secrets"),
            &MasterKey::from_bytes(self.master),
        )
        .unwrap();
        KeeperKeys::load_or_create(&secrets).unwrap()
    }
}

pub struct Harness {
    pub net: Arc<MemoryNetwork>,
    pub clock: Arc<ManualClock>,
    pub config: KeeperConfig,
    pub mailbox: Option<Arc<MemoryMailbox>>,
}

impl Harness {
    pub fn new(config: KeeperConfig) -> Self {
        Self {
            net: MemoryNetwork::new(),
            clock: Arc::new(ManualClock::default()),
            config,
            mailbox: None,
        }
    }

    /// A harness whose keepers share an in-memory relay mailbox.
    pub fn with_mailbox(config: KeeperConfig) -> Self {
        Self {
            mailbox: Some(Arc::new(MemoryMailbox::default())),
            ..Self::new(config)
        }
    }

    pub fn keeper(&self, seed: u8) -> TestKeeper {
        let dir = tempfile::tempdir().unwrap();
        let master = [seed; 32];
        let keeper = self.start(dir.path(), master);
        TestKeeper {
            keeper,
            dir,
            master,
        }
    }

    /// (Re)starts a keeper on `dir`.
    pub fn start(&self, dir: &Path, master: [u8; 32]) -> Keeper {
        let secrets =
            SecretStore::open(dir.join("secrets"), &MasterKey::from_bytes(master)).unwrap();
        let keys = KeeperKeys::load_or_create(&secrets).unwrap();
        let store = Store::open(&dir.join("keeper.db")).unwrap();
        let (transport, inbound) = self
            .net
            .endpoint(keys.identity.public(), keys.transport_public.0);
        Keeper::start(KeeperParts {
            keys,
            secrets,
            store,
            transport: Box::new(transport),
            inbound,
            clock: self.clock.clone(),
            config: self.config.clone(),
            addresses: Vec::new(),
            mailbox: self.mailbox.clone().map(|m| m as Arc<dyn MailboxService>),
        })
        .unwrap()
    }
}

/// Polls `check` until it returns `Some` (10 s budget).
pub async fn eventually<T>(mut check: impl AsyncFnMut() -> Option<T>) -> T {
    for _ in 0..1000 {
        if let Some(value) = check().await {
            return value;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("condition not reached in time");
}

pub fn status(keeper: &Keeper, session: [u8; 32]) -> String {
    keeper
        .ceremony(session)
        .map(|c| c.status)
        .unwrap_or_default()
}

/// Runs a DKG among `keepers` and returns its result.
pub async fn run_dkg(keepers: &[&TestKeeper], threshold: u32, session: [u8; 32]) -> DkgResult {
    let params = DkgParams {
        session,
        threshold,
        members: keepers.iter().map(|k| k.participant()).collect(),
    };
    for k in keepers {
        k.keeper.start_dkg(params.clone()).await.unwrap();
    }
    for k in keepers {
        eventually(async || (status(&k.keeper, session) == "completed").then_some(())).await;
    }
    serde_json::from_value(keepers[0].keeper.ceremony(session).unwrap().result.unwrap()).unwrap()
}

/// A vault manifest for a DKG result, with signer keys whose secrets are returned.
pub fn manifest(
    keepers: &[&TestKeeper],
    result: &DkgResult,
    seed: u64,
) -> (Manifest, Vec<SecretScalar>) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let preaddress = Base::from(seed + 1000);
    let group_key = Point::try_from(&result.group_key).unwrap();
    let context = AddressContext::new(preaddress, &group_key).unwrap();
    let signers: Vec<SecretScalar> = keepers.iter().map(|_| random_scalar(&mut rng)).collect();
    let members = keepers
        .iter()
        .zip(&result.verification_shares)
        .zip(&signers)
        .zip(1..)
        .map(|(((k, share), signer), index)| Member {
            index,
            signer_key: WirePoint::from(&Point::generator_mul_secret(signer).unwrap()),
            verification_share: *share,
            identity: k.keeper.identity(),
            transport: k.keeper.transport_key(),
            addresses: Vec::new(),
        })
        .collect();
    let manifest = Manifest {
        vault: field(&context.address()),
        epoch: 0,
        threshold: 2,
        viewing_threshold: result.threshold,
        preaddress: field(&preaddress),
        group_key: result.group_key,
        members,
    };
    (manifest, signers)
}

/// A random input point.
pub fn point(seed: u64) -> WirePoint {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    WirePoint::from(&Point::generator_mul_secret(&random_scalar(&mut rng)).unwrap())
}

/// `address_secret · P` for each point, combined from a keeper's ECDH batch.
pub async fn ecdh(
    keeper: &Keeper,
    vault: [u8; 32],
    epoch: u32,
    points: &[WirePoint],
) -> Vec<Point> {
    let gathered = keeper
        .ecdh_batch(vault, epoch, EcdhPurpose::NoteDecryption, points.to_vec())
        .await
        .unwrap();
    (0..points.len())
        .map(|i| {
            let partials: Vec<Partial> = gathered.iter().map(|m| m.partials[i]).collect();
            combine(&partials, partials.len()).unwrap()
        })
        .collect()
}
