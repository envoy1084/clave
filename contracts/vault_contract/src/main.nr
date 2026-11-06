pub mod authorization;
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
        authwit::{
            account::AccountActions,
            auth::{compute_authwit_message_hash, compute_authwit_nullifier},
            entrypoint::app::AppPayload,
        },
        context::PrivateContext,
        hash::compute_siloed_nullifier,
        macros::{
            functions::{allow_phase_change, external, initializer, noinitcheck, view},
            storage::storage,
        },
        messages::delivery::MessageDelivery,
        note::note_viewer_options::NoteViewerOptions,
        oracle::{
            auth_witness::get_auth_witness,
            get_nullifier_membership_witness::get_low_nullifier_membership_witness,
        },
        protocol::address::AztecAddress,
        state_vars::{Owned, PrivateSet, SinglePrivateMutable, StateVariable},
    };

    use crate::{
        authorization,
        committee::Committee,
        constants::{MAX_SIGNERS, WITNESS_LEN},
        notes::{config_note::ConfigNote, signer_note::SignerNote},
        point::Point,
        signer_set::SignerSet,
        witness::ApprovalWitness,
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

    // @dev: Keep this signature in sync with the default account entrypoint ABI used by Aztec.js.
    #[external("private")]
    #[noinitcheck]
    #[allow_phase_change]
    fn entrypoint(app_payload: AppPayload, fee_payment_method: u8, cancellable: bool) {
        let actions = AccountActions::init(self.context, is_valid_impl);
        actions.entrypoint(app_payload, fee_payment_method, cancellable);
    }

    #[external("private")]
    #[noinitcheck]
    #[view]
    fn verify_private_authwit(inner_hash: Field) -> Field {
        let actions = AccountActions::init(self.context, is_valid_impl);
        actions.verify_private_authwit(inner_hash)
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

    /// Whether the auth witness held by the PXE approves `inner_hash` for `consumer` and has not been consumed yet.
    ///
    /// Reverts when the witness is malformed (wrong signer set, invalid signature, expired) rather than returning
    /// `false`.
    #[external("utility")]
    unconstrained fn lookup_validity(consumer: AztecAddress, inner_hash: Field) -> bool {
        let config = self.storage.config.view_note();
        let block_header = self.context.block_header();
        let message_hash = compute_authwit_message_hash(
            consumer,
            self.context.chain_id(),
            self.context.version(),
            inner_hash,
        );

        let fields: [Field; WITNESS_LEN] = get_auth_witness(message_hash);
        let witness = ApprovalWitness::from_fields(fields, config.signer_count);
        let approved = authorization::check_witness(
            config,
            witness,
            self.address,
            message_hash,
            block_header.timestamp(),
        );

        let nullifier = compute_authwit_nullifier(self.address, inner_hash);
        let siloed_nullifier = compute_siloed_nullifier(consumer, nullifier);
        let (low_leaf_preimage, _witness) =
            get_low_nullifier_membership_witness(block_header, siloed_nullifier);
        let is_spent = low_leaf_preimage.nullifier == siloed_nullifier;

        approved & !is_spent
    }

    #[contract_library_method]
    fn is_valid_impl(context: &mut PrivateContext, outer_hash: Field) -> bool {
        let config_slot = Storage::init(context).config.get_storage_slot();
        authorization::is_authorized(context, config_slot, outer_hash)
    }
}
