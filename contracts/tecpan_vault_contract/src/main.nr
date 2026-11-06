pub mod committee;
pub mod constants;
pub mod notes;
pub mod point;
pub mod signer_set;
pub mod witness;

use aztec::macros::aztec;

/// Tecpan vault: private k-of-n account contract.
///
/// Stores the spending policy (signer set, threshold `k`) and the viewing committee commitment (threshold `t_v`,
/// verification shares) in private notes owned by the vault. See `architecture/contract.md`.
#[aztec]
pub contract TecpanVault {
    use aztec::{
        macros::{functions::{external, initializer}, storage::storage},
        messages::delivery::MessageDelivery,
        note::note_viewer_options::NoteViewerOptions,
        state_vars::{Owned, PrivateSet, SinglePrivateMutable},
    };

    use crate::{
        committee::Committee,
        constants::MAX_SIGNERS,
        notes::{config_note::ConfigNote, signer_note::SignerNote},
        point::Point,
        signer_set::SignerSet,
    };

    #[storage]
    struct Storage<Context> {
        config: SinglePrivateMutable<ConfigNote, Context>,
        signers: Owned<PrivateSet<SignerNote, Context>, Context>,
    }

    /// Creates the vault for members `0..signer_count`: their signing keys, the spending threshold `k`, the viewing
    /// threshold `t_v` and their viewing verification shares from the distributed key generation.
    #[external("private")]
    #[initializer]
    fn constructor(
        signer_keys: [Point; MAX_SIGNERS],
        signer_count: u32,
        threshold: u32,
        viewing_threshold: u32,
        committee_shares: [Point; MAX_SIGNERS],
    ) {
        let signers = SignerSet::new(signer_keys, signer_count);
        assert((threshold >= 1) & (threshold <= signer_count), "threshold out of range");
        let committee = Committee::new(viewing_threshold, committee_shares, signer_count);

        // Unconstrained delivery: the deployer is a member and every member verifies the delivered config against the
        // agreed manifest before funding the vault, so a bad delivery only bricks an empty vault. Constrained delivery
        // of `MAX_SIGNERS + 1` notes would also exceed the per-call kernel limits.
        //
        // The deployer is usually a separate fee-paying account. Tagging notes with the vault itself as sender lets
        // every member discover them by registering the vault.
        let delivery = MessageDelivery::onchain_unconstrained().with_sender(self.address);

        let config = ConfigNote {
            threshold,
            signer_count,
            version: 0,
            signer_set_commitment: signers.commitment(),
            viewing_threshold,
            committee_commitment: committee.commitment(),
        };
        self.storage.config.initialize(config, self.address).deliver(delivery);

        for slot in 0..MAX_SIGNERS {
            if slot < signer_count {
                let key = signers.keys[slot];
                self
                    .storage
                    .signers
                    .at(self.address)
                    .insert(SignerNote { x: key.x, y: key.y, slot })
                    .deliver(delivery);
            }
        }
    }

    /// Current policy: thresholds, signer count, config version and commitments.
    #[external("utility")]
    unconstrained fn get_config() -> ConfigNote {
        self.storage.config.view_note()
    }

    /// The ordered signer set, rebuilt from signer notes. Members need it to assemble the auth witness.
    #[external("utility")]
    unconstrained fn get_signer_set() -> SignerSet {
        let config = self.storage.config.view_note();
        let notes = self.storage.signers.at(self.address).view_notes(NoteViewerOptions::new());

        let mut keys = [Point::empty(); MAX_SIGNERS];
        for i in 0..notes.len() {
            let note = notes.get(i);
            keys[note.slot] = Point { x: note.x, y: note.y };
        }
        SignerSet { keys, count: config.signer_count }
    }
}
