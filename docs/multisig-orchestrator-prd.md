# Multisig Orchestrator — PRD

## Problem Statement

The Radix Foundation is winding down in 2026. Community funds must move into a DAO-controlled multisig account. There is currently no production application for DAO members to create, sign, and submit multisig transactions. The Rust POC proves the sub-intent approach works on Stokenet, but there is no user-facing system for proposal management, signature collection, or transaction submission.

DAO representatives need a way to:
- Propose arbitrary actions on the multisig treasury (withdrawals, access rule changes, any method call)
- Review and sign proposals via the Radix Wallet
- Track signature progress toward the required threshold
- Submit completed proposals to the network with a fee payer

## Solution

A two-component system:

1. **Rust Backend** — Standalone HTTP server that manages proposal lifecycle, builds sub-intents from raw manifests, collects and validates signatures, monitors proposal validity, and submits composed transactions to the Radix network.

2. **TanStack Start Frontend** — Web application (React 19 + Effect) providing a UI for creating proposals, reviewing manifests, signing via wallet, and submitting transactions.

The backend reads the multisig account's access rule directly from the ledger to determine the current signer set and threshold. A background monitor periodically re-validates pending proposals against access rule changes and signature expiry.

## User Stories

1. As a community member, I want to create a proposal with an arbitrary transaction manifest, so that I can propose any action for the multisig to execute.
2. As a community member, I want to see a list of all proposals and their current status, so that I know what needs my attention.
3. As a DAO signer, I want to view the full manifest text of a proposal, so that I can verify exactly what I'm authorizing before signing.
4. As a DAO signer, I want to sign a proposal via my Radix Wallet using sendPreAuthorizationRequest, so that my signature is collected securely.
5. As a DAO signer, I want to see which signers have already signed a proposal and how many more are needed, so that I can track progress toward the threshold.
6. As a fee payer, I want to submit a proposal that has reached its signature threshold, paying fees from my own wallet account, so that the transaction is executed on the network.
7. As a community member, I want to see the final transaction status (committed/failed) after submission, so that I know whether the proposal was executed.
8. As a DAO signer, I want to be warned if a proposal has become invalid due to access rule changes or signature expiry, so that I don't waste time on unexecutable proposals.
9. As a community member, I want proposals to have an expiry encoded in the sub-intent manifest, so that old proposals cannot be executed indefinitely.
10. As a fee payer, I want the system to preview/validate the composed transaction before I sign and submit, so that I don't waste fees on a failing transaction.
11. As a DAO signer, I want the orchestrator to only accept signatures from accounts in the current access rule, so that the signature limit isn't exhausted by unauthorized signers.
12. As a community member, I want to see the multisig account's current access rule (signers and threshold), so that I know who needs to sign.
13. As a community member, I want to set an expiry when creating a proposal, so that the sub-intent has a network-enforced deadline.
14. As a community member, I want to see proposals that have expired or failed, so that I have a full history of multisig activity.
15. As a DAO signer, I want the system to re-validate proposals in the background and flag ones that became invalid, so that I'm not signing dead proposals.

## Implementation Decisions

### Architecture

- **Rust backend**: Standalone HTTP server using `radixdlt-scrypto` crate for transaction/manifest building, `axum` or similar for HTTP, `sqlx` for PostgreSQL.
- **Frontend**: TanStack Start app with Effect for services, Radix dApp Toolkit for wallet integration.
- **Storage**: PostgreSQL for proposals, signatures, and status.
- **Communication**: Frontend calls Rust backend via REST. HTTP polling for status updates (no WebSocket in v1).

### Modules (Backend)

1. **ProposalStore** — PostgreSQL-backed proposal CRUD + state machine.
   - States: `Created → Signing → Ready → Submitting → Committed | Failed | Expired | Invalid`
   - Stores: proposal metadata, raw manifest, epoch window, collected signatures, submission result.
   - Deep module: encapsulates all DB queries, state transition validation, and expiry logic behind simple create/get/list/update_status interface.

2. **TransactionBuilder** — Builds sub-intents from raw manifests, composes main transactions with fee payer.
   - Input: raw manifest string + config (epoch window, treasury account, fee payer account).
   - Output: unsigned sub-intent (for signing), composed main transaction (for submission).
   - Reuses POC patterns: `PartialTransactionV2`, `TransactionV2Builder`, `yield_to_child`/`yield_to_parent`.
   - Deep module: hides all Scrypto SDK complexity behind build_subintent() and compose_main_transaction() interface.

3. **SignatureCollector** — Receives, validates, and stores signatures.
   - Validates: signer is in the current access rule, signature is cryptographically valid against the sub-intent hash, signature hasn't expired.
   - Tracks threshold progress.
   - Deep module: threshold logic, deduplication, and validation behind add_signature() / get_status() interface.

4. **GatewayClient** — Radix Gateway API client.
   - Read access rule from account metadata/state.
   - Submit transactions, poll for commit status.
   - Preview composed transactions before submission.
   - Get current epoch for validity windows.
   - Extends POC's `gateway.rs` with access rule reading and preview.

5. **ValidityMonitor** — Background task that periodically checks pending proposals.
   - Detects: access rule changes that invalidate collected signatures, expired signatures.
   - Updates proposal status to `Invalid` or flags affected signatures.
   - Runs on a configurable interval.

6. **API Layer** — REST endpoints (thin delegation to above modules):
   - `POST /proposals` — Create proposal (manifest text + config)
   - `GET /proposals` — List proposals with status
   - `GET /proposals/:id` — Get proposal detail
   - `POST /proposals/:id/sign` — Submit a signature
   - `POST /proposals/:id/prepare` — Build main tx for fee payer
   - `POST /proposals/:id/submit` — Add fee payer signature, notarize, submit
   - `GET /account/access-rule` — Current multisig access rule

### Modules (Frontend)

1. **OrchestratorClient** (Effect service) — HTTP client wrapping all backend API calls.
2. **WalletService** (Effect service) — Wraps Radix dApp Toolkit: `sendPreAuthorizationRequest` for sub-intent signing, standard wallet connect for fee payment signing.
3. **Pages**: Proposal List (`/`), Proposal Detail (`/proposals/:id`), Create Proposal (`/proposals/new`).

### Key Design Decisions

- **Raw manifest input**: Proposers paste transaction manifest text. No UI builder in v1. This maximizes flexibility for arbitrary actions.
- **Signer restriction**: Backend rejects signatures from accounts not in the current access rule (prevents signature limit exhaustion attack described in architecture doc).
- **No cancellation**: Proposals can only expire. Simplifies state machine. If access rules change, the validity monitor flags the proposal.
- **Fee payer = submitter**: The user who clicks "submit" connects their wallet and pays fees. No service account.
- **Single multisig account**: Configured via environment variable. Multi-account support deferred.
- **Access rule from ledger**: Backend reads the account's access rule from the Gateway API. No manual signer configuration.
- **Expiry in manifest**: Proposal epoch window is set at creation time and encoded in the sub-intent, enforced by the network.

### Proposal Submission Flow

1. Proposer creates proposal → backend stores manifest + builds unsigned sub-intent
2. Signers view proposal → see full manifest → sign via wallet (`sendPreAuthorizationRequest`)
3. Frontend sends signed sub-intent hex to backend → backend extracts and stores signature
4. Once threshold met → proposal status becomes `Ready`
5. Fee payer clicks "Submit" → backend composes main tx (lock_fee + yield_to_child) → returns tx for fee payer to sign
6. Fee payer signs via wallet → frontend sends signature to backend
7. Backend notarizes → submits to network → polls for result → updates status

### Database Schema (Conceptual)

- **proposals**: id, manifest_text, treasury_account, epoch_min, epoch_max, status, subintent_hash, created_at, submitted_at, tx_id
- **signatures**: id, proposal_id, signer_public_key, signature_bytes, created_at
- **submission_attempts**: id, proposal_id, fee_payer_account, tx_hash, status, created_at

## Testing Decisions

### What Makes a Good Test

Tests should verify external behavior through public interfaces, not implementation details. A test should break only when the module's contract changes, not when internal refactoring occurs.

### Modules to Test (Backend Only)

1. **TransactionBuilder** — Test with deterministic keys (same approach as POC). Verify sub-intent construction from raw manifests, main transaction composition, correct epoch windows. Can test entirely offline.

2. **SignatureCollector** — Test threshold logic, duplicate rejection, access rule validation. Mock the GatewayClient to provide access rules.

3. **ProposalStore** — Test state machine transitions (valid and invalid). Use a test PostgreSQL instance.

4. **ValidityMonitor** — Test detection of access rule changes and signature expiry. Mock GatewayClient and ProposalStore.

5. **API Layer** — Integration tests hitting real endpoints with a test database. Verify request/response contracts.

6. **GatewayClient** — Unit tests with mocked HTTP responses. Integration tests against Stokenet (marked `#[ignore]` like POC).

### Prior Art

The POC (`multisig-poc/`) has unit tests in each module demonstrating the deterministic key / offline testing pattern.

## Out of Scope

- **Notifications** — No email/push notifications when proposals need signing. Signers poll the UI.
- **WebSocket / real-time updates** — HTTP polling only in v1.
- **Multiple multisig accounts** — Single configured account only.
- **Manifest builder UI** — Raw text input only. No visual or template-based manifest creation.
- **Proposal cancellation** — Proposals expire or become invalid; no explicit cancel action.
- **Deployment architecture** — Not specified in this PRD.
- **Frontend tests** — Only backend modules are tested.
- **Vote delegation integration** — No connection to the Consultation v2 governance system.
- **Mobile support** — Desktop-first. Responsive design not required in v1.

## Further Notes

- The backend should reuse patterns from the POC (`multisig-poc/src/`) — particularly `subintent.rs` (sub-intent building), `transaction.rs` (main tx composition), and `gateway.rs` (API client).
- The `radixdlt-scrypto` crate is used via local path in the POC. For production, evaluate whether to publish or continue local path references.
- The architecture doc (`docs/multisig-architecture.md`) remains the authoritative reference for the sub-intent flow, security considerations, and transaction structure.
- The `sendPreAuthorizationRequest` wallet API returns a signed sub-intent hex. The backend must parse this to extract the signature and signer public key.
- `intent_discriminator` must use cryptographically random values in production (POC uses `SystemTime::now().as_nanos()` which has collision risk).

## Unresolved Questions

1. **File location**: `docs/multisig-architecture.md` already contains the architecture doc. Should this PRD go to a new file like `docs/multisig-orchestrator-prd.md`? Or also/instead as a GitHub issue?
2. **Rust HTTP framework**: `axum` assumed — any preference?
3. **CORS / auth**: How does the frontend authenticate to the Rust backend? Open API? API key? Or rely on wallet signatures as proof of identity?
4. **Manifest validation**: Should the backend validate the raw manifest text before creating a proposal (e.g. parse it, check it references the correct treasury account)?
5. **How does the backend read access rules from the ledger?** The Gateway API's entity details endpoint returns account metadata, but the access rule structure may need specific parsing from the Scrypto `AccessRule` type.
