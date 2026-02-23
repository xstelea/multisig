# Multisig Orchestrator — GitHub Issues

Parent PRD: `docs/multisig-orchestrator-prd.md`

---

## Issue 1: Project scaffolding + access rule display

### What to build

The tracer bullet. Stand up the full stack end-to-end and deliver the first visible feature: displaying the multisig account's current access rule (signers and threshold).

**Backend (Rust/axum):**
- New crate `multisig-server` with axum HTTP server
- PostgreSQL connection pool (sqlx) + migration runner (empty initial migration)
- CORS configuration for frontend origin
- `GatewayClient` module — extend POC's `gateway.rs` with `read_access_rule(account_address)` that fetches the account's access rule from the Gateway API (`/state/entity/details`) and parses the signer set + threshold
- `GET /health` endpoint
- `GET /account/access-rule` endpoint returning `{ signers: [{ public_key, badge }], threshold: number }`
- Configuration via environment variables: `MULTISIG_ACCOUNT_ADDRESS`, `GATEWAY_URL`, `DATABASE_URL`, `FRONTEND_ORIGIN`

**Frontend (TanStack Start):**
- New app with TanStack Start + React 19 + Tailwind CSS v4
- Effect runtime setup (Atom context + global layer)
- Radix dApp Toolkit initialization (connect button, network config)
- `OrchestratorClient` Effect service — HTTP client wrapping backend API calls (start with `/health` and `/account/access-rule`)
- Home page (`/`) displaying: connected wallet state, multisig account address, current signers with public keys, required threshold (e.g. "3 of 4 signatures required")

**Reuses from POC:** `gateway.rs` patterns (reqwest client, response types, bech32 encoding). Reference `context/consultation.md` for Effect + TanStack Start patterns.

### Acceptance criteria

- [ ] Rust backend starts on configurable port, connects to PostgreSQL, runs migrations
- [ ] `GET /health` returns 200
- [ ] `GET /account/access-rule` returns the signer set and threshold for the configured multisig account (read from Stokenet)
- [ ] Frontend loads, initializes Radix dApp Toolkit connect button
- [ ] Home page displays the multisig account's signers and threshold fetched from the backend
- [ ] CORS allows frontend to call backend
- [ ] Backend has unit tests for access rule parsing; GatewayClient has `#[ignore]` integration test against Stokenet

### Blocked by

None — can start immediately.

### User stories addressed

- User story 12: See the multisig account's current access rule (signers and threshold)

---

## Issue 2: Create & view proposals

### What to build

Proposers can paste a raw transaction manifest, set an expiry, and create a proposal. All users can browse the proposal list and view proposal details including the full manifest text.

**Database:**
- `proposals` table migration: `id` (UUID), `manifest_text`, `treasury_account`, `epoch_min`, `epoch_max`, `status` (enum: Created, Signing, Ready, Submitting, Committed, Failed, Expired, Invalid), `subintent_hash`, `created_at`, `submitted_at`, `tx_id`

**Backend:**
- `ProposalStore` module — PostgreSQL-backed CRUD with state machine. Create stores manifest + computes epoch window from expiry config. Get/list with status filtering. State transition validation (only valid transitions allowed).
- `TransactionBuilder` module — `build_unsigned_subintent(manifest_text, config)` that takes raw manifest text, wraps it with the correct epoch window and intent discriminator (cryptographically random), and returns the unsigned `PartialTransactionV2` + `SubintentHash`. Reuses POC's `subintent.rs` patterns.
- API endpoints:
  - `POST /proposals` — accepts `{ manifest_text, expiry_epoch }`, builds unsigned subintent, stores proposal, returns proposal with subintent hash
  - `GET /proposals` — list all proposals with status
  - `GET /proposals/:id` — full proposal detail including manifest text, epoch window, status, subintent hash

**Frontend:**
- Create Proposal page (`/proposals/new`) — textarea for manifest text, epoch/expiry input, submit button. On success, redirect to proposal detail.
- Proposal List page (`/`) — table/cards showing all proposals with status badges, created date, epoch window. Link to detail.
- Proposal Detail page (`/proposals/:id`) — full manifest text display (monospace/code block), status badge, epoch window, subintent hash, created date.
- `OrchestratorClient` — add `createProposal()`, `listProposals()`, `getProposal(id)` methods.

**Key decisions from PRD:**
- Raw manifest input only (no builder UI)
- Expiry encoded in sub-intent manifest (network-enforced)
- `intent_discriminator` must use cryptographically random values (not `SystemTime::now()` like POC)

### Acceptance criteria

- [ ] `POST /proposals` accepts manifest text + expiry, builds unsigned subintent, stores in DB, returns proposal
- [ ] `GET /proposals` returns all proposals with status
- [ ] `GET /proposals/:id` returns full proposal detail
- [ ] Subintent hash is computed and stored at creation time
- [ ] Intent discriminator uses cryptographically random value
- [ ] Proposal status starts as `Created`
- [ ] ProposalStore validates state transitions (rejects invalid ones)
- [ ] Frontend Create page submits manifest and redirects to detail
- [ ] Frontend List page shows all proposals with status
- [ ] Frontend Detail page displays full manifest text, status, epoch window
- [ ] ProposalStore has unit tests for CRUD and state machine transitions (test PostgreSQL)
- [ ] TransactionBuilder has unit tests for subintent construction (offline, deterministic keys)

### Blocked by

- Blocked by Issue 1 (project scaffolding, GatewayClient, frontend app shell)

### User stories addressed

- User story 1: Create a proposal with an arbitrary transaction manifest
- User story 2: See a list of all proposals and their current status
- User story 3: View the full manifest text of a proposal
- User story 9: Proposals have expiry encoded in the sub-intent manifest
- User story 13: Set an expiry when creating a proposal

---

## Issue 3: Collect signatures

### What to build

DAO signers can sign a proposal via their Radix Wallet. The backend validates each signature against the current access rule, stores it, tracks progress toward the threshold, and transitions the proposal to `Ready` when the threshold is met.

**Database:**
- `signatures` table migration: `id` (UUID), `proposal_id` (FK), `signer_public_key`, `signature_bytes`, `created_at`

**Backend:**
- `SignatureCollector` module:
  - `add_signature(proposal_id, signed_partial_transaction_hex)` — parses the signed partial transaction hex (returned by wallet's `sendPreAuthorizationRequest`), extracts the signature and signer public key, validates:
    1. Signer's badge is in the current access rule (calls GatewayClient)
    2. Signature is cryptographically valid against the stored subintent hash
    3. No duplicate signature from same signer
  - `get_signature_status(proposal_id)` — returns collected signatures, total needed, signers who've signed, remaining signers
  - When threshold is met: transition proposal status `Signing → Ready`
- State transition: `Created → Signing` on first signature, `Signing → Ready` when threshold met
- API endpoint: `POST /proposals/:id/sign` — accepts `{ signed_partial_transaction_hex }`, returns updated signature status

**Frontend:**
- Proposal Detail page additions:
  - "Sign" button (visible when wallet connected and user is a valid signer who hasn't signed yet)
  - Sign flow: click → `walletApi.sendPreAuthorizationRequest(subintentManifest)` → receive signed partial hex → `POST /proposals/:id/sign`
  - Signature progress display: list of signers with signed/unsigned status, "3 of 4 signatures collected" progress indicator
  - Status badge updates: Created → Signing → Ready
- `WalletService` Effect service — wraps Radix dApp Toolkit's `sendPreAuthorizationRequest`. Reference `context/radix-radix-dapp-toolkit.md` for the API.
- `OrchestratorClient` — add `signProposal(id, signedHex)` method

**Key decisions from PRD:**
- Only accept signatures from accounts in the current access rule (prevents signature limit exhaustion — see architecture doc)
- Backend parses `sendPreAuthorizationRequest` response hex to extract signature + public key
- Wallet returns `SignedPartialTransactionV2` hex; backend must decode this

### Acceptance criteria

- [ ] `POST /proposals/:id/sign` accepts signed partial transaction hex, extracts and stores signature
- [ ] Rejects signatures from accounts not in the current access rule
- [ ] Rejects duplicate signatures from the same signer
- [ ] Validates signature cryptographically against the subintent hash
- [ ] Transitions proposal Created → Signing on first signature
- [ ] Transitions proposal Signing → Ready when threshold met
- [ ] API returns current signature status (who signed, how many more needed)
- [ ] Frontend Sign button triggers `sendPreAuthorizationRequest` via wallet
- [ ] Frontend displays signature progress (signed/unsigned per signer, count)
- [ ] SignatureCollector has unit tests (mock GatewayClient for access rule, test threshold logic, duplicate rejection, invalid signer rejection)

### Blocked by

- Blocked by Issue 2 (proposals must exist to sign them)

### User stories addressed

- User story 4: Sign a proposal via Radix Wallet using sendPreAuthorizationRequest
- User story 5: See which signers have signed and how many more are needed
- User story 11: Only accept signatures from accounts in the current access rule

---

## Issue 4: Submit transaction

### What to build

When a proposal reaches `Ready` status, a fee payer can preview the composed transaction, sign it via their wallet, and submit it to the network. The system displays the final transaction status.

**Database:**
- `submission_attempts` table migration: `id` (UUID), `proposal_id` (FK), `fee_payer_account`, `tx_hash`, `status`, `created_at`

**Backend:**
- `TransactionBuilder` additions:
  - `compose_main_transaction(proposal_id, fee_payer_account)` — retrieves the proposal's signed subintent (with all collected signatures), builds the main transaction manifest (`lock_fee(fee_payer, amount)` + `yield_to_child("withdrawal", subintent)`), returns the unsigned main transaction for the fee payer to sign. Reuses POC's `transaction.rs` patterns.
- `GatewayClient` additions:
  - `preview_transaction(compiled_tx_hex)` — calls Gateway preview endpoint to validate the composed transaction before submission
  - `submit_transaction(compiled_tx_hex)` — submit to network
  - `poll_transaction_status(intent_hash)` — poll until committed/failed (reuse POC's `wait_for_commit` pattern)
- API endpoints:
  - `POST /proposals/:id/prepare` — compose main tx, preview it, return unsigned tx for fee payer signing. Returns preview result (success/failure prediction).
  - `POST /proposals/:id/submit` — accepts fee payer's signature, notarizes the main transaction, submits to network, polls for result, updates proposal status
- State transitions: `Ready → Submitting` on submit, `Submitting → Committed` or `Submitting → Failed` based on network result
- Store submission attempt with tx hash and status

**Frontend:**
- Proposal Detail page additions:
  - "Submit" button (visible when proposal is `Ready` and wallet is connected)
  - Submit flow:
    1. Click Submit → `POST /proposals/:id/prepare` → receive preview result + unsigned tx
    2. Display preview result (will succeed / will fail)
    3. If preview succeeds → `walletApi.sendTransaction(mainTxManifest)` → fee payer signs
    4. Send fee payer signature → `POST /proposals/:id/submit`
    5. Display submission progress → final status (Committed / Failed)
  - Transaction status display: tx hash (linked to Radix dashboard), committed/failed status, error message if failed
- `OrchestratorClient` — add `prepareSubmission(id)`, `submitProposal(id, feePayerSignature)` methods

**Key decisions from PRD:**
- Fee payer = submitter (no service account)
- Preview/validate before submission is mandatory (user story 10)
- `notary_is_signatory=true` so notary signature authorizes `lock_fee` (POC pattern)

### Acceptance criteria

- [ ] `POST /proposals/:id/prepare` composes main transaction with signed subintent, previews via Gateway, returns result
- [ ] Preview catches failing transactions before submission
- [ ] `POST /proposals/:id/submit` notarizes, submits, polls for result, updates proposal status
- [ ] Proposal transitions Ready → Submitting → Committed or Failed
- [ ] Submission attempt is recorded with tx hash and status
- [ ] Frontend Submit button triggers prepare → preview → wallet sign → submit flow
- [ ] Frontend displays transaction status with link to Radix dashboard
- [ ] Frontend shows error details if transaction fails
- [ ] TransactionBuilder has unit tests for main tx composition (offline, deterministic keys)
- [ ] GatewayClient has `#[ignore]` integration tests for preview and submit

### Blocked by

- Blocked by Issue 3 (proposals must have collected signatures to submit)

### User stories addressed

- User story 6: Submit a proposal that has reached its threshold, paying fees from own wallet
- User story 7: See the final transaction status after submission
- User story 10: Preview/validate the composed transaction before signing and submitting

---

## Issue 5: Validity monitoring

### What to build

A background task periodically checks pending proposals and flags those that have become invalid due to access rule changes or epoch expiry. Users see warnings on affected proposals and can view expired/failed proposals in the history.

**Backend:**
- `ValidityMonitor` module — background task (tokio interval) that:
  1. Fetches all proposals in `Created`, `Signing`, or `Ready` status
  2. For each, checks:
     - **Epoch expiry**: current epoch > proposal's `epoch_max` → transition to `Expired`
     - **Access rule changes**: re-fetch access rule from ledger, compare against signers who've signed. If any signer is no longer in the access rule, flag their signatures as invalid. If remaining valid signatures < threshold, transition to `Invalid`.
  3. Runs on a configurable interval (e.g. every 30 seconds, configurable via env var)
- `ProposalStore` additions:
  - List proposals by status (for monitor queries)
  - `mark_expired(proposal_id)`, `mark_invalid(proposal_id, reason)`
  - Support for flagging individual signatures as invalid (without deleting them)
- `GET /proposals` and `GET /proposals/:id` — include `invalid_reason` field when status is Invalid, include signature validity flags

**Frontend:**
- Proposal Detail page additions:
  - Warning banner when proposal is `Expired` or `Invalid` with reason (e.g. "Access rule changed — signer X removed", "Proposal expired at epoch N")
  - Invalidated signatures shown with strikethrough or warning icon
- Proposal List page additions:
  - Expired and Invalid proposals visible in the list with distinct status badges
  - Optional filter/tab: Active vs. All (including expired/failed/invalid)

### Acceptance criteria

- [ ] Background task runs on configurable interval
- [ ] Detects proposals past their epoch window and marks them `Expired`
- [ ] Detects access rule changes that invalidate collected signatures
- [ ] Marks proposals `Invalid` when remaining valid signatures fall below threshold
- [ ] Individual signatures can be flagged as invalid (signer removed from access rule)
- [ ] API responses include invalid reason and signature validity flags
- [ ] Frontend shows warning banner on Invalid/Expired proposals
- [ ] Frontend shows expired/failed/invalid proposals in list with appropriate badges
- [ ] ValidityMonitor has unit tests (mock GatewayClient and ProposalStore, test epoch expiry detection, access rule change detection)

### Blocked by

- Blocked by Issue 3 (needs proposals with signatures to monitor)

### User stories addressed

- User story 8: Be warned if a proposal has become invalid due to access rule changes or signature expiry
- User story 14: See proposals that have expired or failed (full history)
- User story 15: Background re-validation of pending proposals

---

## Summary

| Issue | Title | Blocked by | Parallelizable with |
|-------|-------|------------|---------------------|
| 1 | Project scaffolding + access rule display | None | — |
| 2 | Create & view proposals | Issue 1 | — |
| 3 | Collect signatures | Issue 2 | — |
| 4 | Submit transaction | Issue 3 | Issue 5 |
| 5 | Validity monitoring | Issue 3 | Issue 4 |

```
Issue 1 → Issue 2 → Issue 3 → Issue 4
                            ↘ Issue 5
```

All 15 user stories from the PRD are covered:
- Issue 1: US 12
- Issue 2: US 1, 2, 3, 9, 13
- Issue 3: US 4, 5, 11
- Issue 4: US 6, 7, 10
- Issue 5: US 8, 14, 15
