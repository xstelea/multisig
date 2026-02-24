# Multisig Orchestrator POC Design

A proof-of-concept that validates the end-to-end sub-intent multisig flow on Radix Stokenet.

## Goal

Prove that:

1. A sub-intent can be created for a DAO action (withdraw + deposit)
2. Multiple signatures can be collected off-chain
3. A fee payer can wrap and submit the transaction
4. The 3-of-4 threshold is enforced by the network

## Decisions

| Aspect      | Choice                               |
| ----------- | ------------------------------------ |
| Network     | Stokenet                             |
| Language    | Rust                                 |
| Output      | Single script with step-by-step logs |
| Persistence | None — fully self-contained runs     |
| DAO action  | Withdraw + deposit via worktop       |
| Keys        | Hardcoded testnet keys (pre-funded)  |

## Project Structure

```
multisig-orchestrator-poc/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, orchestrates the flow
│   ├── keys.rs              # Hardcoded testnet private keys
│   ├── accounts.rs          # Account setup + access rule configuration
│   ├── subintent.rs         # Sub-intent creation + signing
│   ├── transaction.rs       # Main tx building, notarizing, submission
│   └── gateway.rs           # Stokenet Gateway API client
```

### Dependencies

- `radix-engine-toolkit` — Transaction building, manifest construction, signing
- `radix-common` — Core Radix types (addresses, decimals, etc.)
- `reqwest` — HTTP client for Gateway API
- `tokio` — Async runtime
- `hex` — Key encoding/decoding
- `anyhow` — Error handling

## Account Setup

```
┌─────────────────────────────────────────────────────────┐
│  DAO Signers (4 accounts) — Pre-funded, hardcoded keys │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐       │
│  │ Signer1 │ │ Signer2 │ │ Signer3 │ │ Signer4 │       │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘       │
│                                                         │
│  Fee Payer (1 account) — Pre-funded, hardcoded key     │
│  ┌─────────────┐                                        │
│  │  FeePayer   │  ← Signs main tx, pays fees           │
│  └─────────────┘                                        │
│                                                         │
│  DAO Treasury (created fresh each run)                  │
│  ┌─────────────┐                                        │
│  │ DAOTreasury │  ← 3-of-4 access rule                 │
│  └─────────────┘                                        │
│                                                         │
│  Recipient (one of the signer accounts)                │
└─────────────────────────────────────────────────────────┘
```

**Phase 0 — Setup Transaction:**

1. Create DAO Treasury account with access rule: `require(3 of [pk1, pk2, pk3, pk4])`
2. Fund it with XRD from a pre-funded account
3. Recipient is one of the signer accounts

## Execution Flow

### Phase 1: Sub-Intent Creation

```
CALL_METHOD
    Address("dao_treasury")
    "withdraw"
    Address("resource_rdx1...")  // XRD
    Decimal("100");

CALL_METHOD
    Address("recipient")
    "deposit_batch"
    Expression("ENTIRE_WORKTOP");

YIELD_TO_PARENT;
```

- Expiry encoded in intent header (1 hour)
- Network enforces expiry, not just app

### Phase 2: Signature Collection

Collect signatures from Signer1, Signer2, Signer3 (skip Signer4 to prove 3-of-4 works):

1. Hash the sub-intent
2. Sign with each private key
3. Store signatures

### Phase 3: Main Transaction Composition

```
USE_CHILD
    NamedIntent("dao_withdraw")
    Intent("<subintent_hash>");

CALL_METHOD
    Address("fee_payer_account")
    "lock_fee"
    Decimal("10");

YIELD_TO_CHILD
    NamedIntent("dao_withdraw");
```

### Phase 4: Submission

1. Compose: main intent + sub-intent with signatures
2. Fee payer signs main intent
3. Notarize (using fee payer key)
4. Submit to Stokenet Gateway (`POST /transaction/submit`)
5. Poll for commit status
6. Verify recipient balance increased

## Console Output

```
═══════════════════════════════════════════════════════════
  MULTISIG ORCHESTRATOR POC — Stokenet
═══════════════════════════════════════════════════════════

[PHASE 0] Account Setup
  • DAO Treasury created: account_tdx_2_...
  • Access rule: 3 of 4 signers
  • Funded with 500 XRD
  ✓ Setup complete

[PHASE 1] Sub-Intent Creation
  • Action: Withdraw 100 XRD → Recipient
  • Expiry: 2026-01-16T15:30:00Z
  • Sub-intent hash: subtxid_tdx_2_...
  ✓ Sub-intent created

[PHASE 2] Signature Collection
  • Signer1: ✓ signed
  • Signer2: ✓ signed
  • Signer3: ✓ signed
  • Signer4: (skipped — threshold already met)
  ✓ 3/4 signatures collected

[PHASE 3] Transaction Composition
  • Fee payer: account_tdx_2_...
  • Lock fee: 10 XRD
  ✓ Main intent built

[PHASE 4] Submission
  • Fee payer signed: ✓
  • Notarized: ✓
  • Submitted: txid_tdx_2_...
  • Status: CommittedSuccess
  ✓ Transaction committed

[RESULT] ══════════════════════════════════════════════════
  DAO Treasury: 400 XRD (was 500)
  Recipient:    +100 XRD
  SUCCESS — Multisig withdrawal completed
═══════════════════════════════════════════════════════════
```

## Error Handling

Uses `anyhow` for errors. If any phase fails, print error and exit. No partial state recovery needed for POC.

## References

- [Multisig Architecture](../multisig-architecture.md)
- [Radix Subintents Reference](../radix-subintents-reference.md)
