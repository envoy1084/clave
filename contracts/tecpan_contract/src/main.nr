pub mod authorization;
pub mod constants;
pub mod notes;
pub mod signer_key;
pub mod signer_set;
pub mod witness;

use aztec::macros::aztec;

/// Private k-of-n multisig account.
///
/// Executes a payload only when the auth witness carries valid Schnorr signatures from at least `threshold` registered
/// signers. The signer set and threshold are held in private notes owned by the account itself; see
/// `architecture/contract.md` for the witness layout and trust model.
#[aztec]
pub contract TecpanMultisig {
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
        constants::{MAX_SIGNERS, WITNESS_LEN},
        notes::{config_note::ConfigNote, signer_note::SignerNote},
        signer_key::SignerKey,
        signer_set::SignerSet,
        witness::ApprovalWitness,
    };

    #[storage]
    struct Storage<Context> {
        config: SinglePrivateMutable<ConfigNote, Context>,
        signers: Owned<PrivateSet<SignerNote, Context>, Context>,
    }

    /// Creates the multisig with signers `signer_keys[0..signer_count]` and the given approval threshold.
    #[external("private")]
    #[initializer]
    fn constructor(signer_keys: [SignerKey; MAX_SIGNERS], signer_count: u32, threshold: u32) {
        let signers = SignerSet::new(signer_keys, signer_count);
        assert((threshold >= 1) & (threshold <= signer_count), "threshold out of range");

        // Unconstrained delivery: the deployer is one of the signers and co-signers verify the delivered config
        // against the agreed signer set before funding the account, so a bad delivery only bricks an empty account.
        // Constrained delivery of up to `MAX_SIGNERS + 1` notes would also exceed the per-call kernel limits.
        //
        // The deployer is usually a separate fee-paying account. Tagging notes with the multisig itself as sender lets
        // every co-signer discover them by registering the multisig address.
        let delivery = MessageDelivery::onchain_unconstrained().with_sender(self.address);

        let config = ConfigNote {
            threshold,
            signer_count,
            version: 0,
            signer_set_commitment: signers.commitment(),
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

    /// Current threshold, signer count, config version and signer set commitment.
    #[external("utility")]
    unconstrained fn get_config() -> ConfigNote {
        self.storage.config.view_note()
    }

    /// The ordered signer set, rebuilt from signer notes. Signers need it to assemble the auth witness.
    #[external("utility")]
    unconstrained fn get_signer_set() -> SignerSet {
        let config = self.storage.config.view_note();
        let notes = self.storage.signers.at(self.address).view_notes(NoteViewerOptions::new());

        let mut keys = [SignerKey::empty(); MAX_SIGNERS];
        for i in 0..notes.len() {
            let note = notes.get(i);
            keys[note.slot] = SignerKey { x: note.x, y: note.y };
        }
        SignerSet { keys, count: config.signer_count }
    }

    /// Whether the auth witness held by the PXE approves `inner_hash` for `consumer` and has not been consumed yet.
    ///
    /// Reverts when the witness is malformed (wrong signer set, invalid signature) rather than returning `false`.
    #[external("utility")]
    unconstrained fn lookup_validity(consumer: AztecAddress, inner_hash: Field) -> bool {
        let config = self.storage.config.view_note();
        let message_hash = compute_authwit_message_hash(
            consumer,
            self.context.chain_id(),
            self.context.version(),
            inner_hash,
        );

        let fields: [Field; WITNESS_LEN] = get_auth_witness(message_hash);
        let witness = ApprovalWitness::from_fields(fields, config.signer_count);
        let approved = authorization::check_witness(config, witness, self.address, message_hash);

        let nullifier = compute_authwit_nullifier(self.address, inner_hash);
        let siloed_nullifier = compute_siloed_nullifier(consumer, nullifier);
        let (low_leaf_preimage, _witness) =
            get_low_nullifier_membership_witness(self.context.block_header(), siloed_nullifier);
        let is_spent = low_leaf_preimage.nullifier == siloed_nullifier;

        approved & !is_spent
    }

    #[contract_library_method]
    fn is_valid_impl(context: &mut PrivateContext, outer_hash: Field) -> bool {
        let config_slot = Storage::init(context).config.get_storage_slot();
        authorization::is_authorized(context, config_slot, outer_hash)
    }
}
