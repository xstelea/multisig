# Multisig Orchestrator

Radix DLT multisig transaction orchestrator — propose, collect signatures, and submit multi-party transactions on Babylon Stokenet.

## Prerequisites

- Docker & Docker Compose
- Rust (2021 edition) + Cargo
- Node.js + pnpm

## Quick Start

```bash
./dev.sh
```

This starts PostgreSQL, the backend, and the frontend in one command.

## Architecture

### How It Works

The server builds transaction subintents from user-provided manifests, and each signer approves via wallet pre-authorization. The server collects signatures, validates them against the account's on-ledger access rule, and — once the threshold is met — assembles a `NotarizedTransactionV2` with a fee-payment child and a withdrawal child, then submits it to the Radix Gateway.

### Signing Flow

```mermaid
sequenceDiagram
    participant Browser
    participant Wallet
    participant Server
    participant Gateway

    Note over Browser,Server: 1. Create Proposal
    Browser->>Server: POST /proposals {manifest_text, expiry_epoch}
    Server->>Gateway: GET current epoch
    Server->>Server: Compile manifest into unsigned PartialTransactionV2
    Server-->>Browser: Proposal (subintent_hash, status: created)

    Note over Browser,Wallet: 2. Collect Signatures (repeat per signer)
    Browser->>Wallet: sendPreAuthorizationRequest(manifest)
    Wallet-->>Browser: SignedPartialTransactionV2 hex
    Browser->>Server: POST /proposals/{id}/sign {signed_partial_transaction_hex}
    Server->>Server: Extract Ed25519 pubkey + signature from hex
    Server->>Server: Hash pubkey → key_hash
    Server->>Gateway: Fetch account access rule
    Server->>Server: Match key_hash against access rule signers
    Server-->>Browser: Signature stored (status: signing → ready when threshold met)

    Note over Browser,Gateway: 3. Submit Transaction
    Browser->>Server: POST /proposals/{id}/prepare {fee_payer_account}
    Server-->>Browser: Fee manifest (lock_fee 10 XRD + YIELD_TO_PARENT)
    Browser->>Wallet: sendPreAuthorizationRequest(fee_manifest)
    Wallet-->>Browser: Signed fee payment hex
    Browser->>Server: POST /proposals/{id}/submit {signed_fee_payment_hex}
    Server->>Server: Reconstruct withdrawal partial (stored unsigned + collected sigs)
    Server->>Server: Compose NotarizedTransactionV2 with both children
    Server->>Gateway: POST /transaction/submit
    Gateway-->>Server: Poll until committed
    Server-->>Browser: status: committed
```

### Transaction Structure

The final `NotarizedTransactionV2` contains two child subintents:

| Child | Manifest | Signed by |
|---|---|---|
| `fee_payment` | `lock_fee("10") → YIELD_TO_PARENT` | Fee payer (at submit time) |
| `withdrawal` | User's proposal manifest → `YIELD_TO_PARENT` | Multisig signers (collected during signing phase) |

The parent transaction manifest simply yields to each child in order:

```
YIELD_TO_CHILD("fee_payment");
YIELD_TO_CHILD("withdrawal");
```

A server-generated ephemeral Ed25519 key notarizes the transaction (`notary_is_signatory: false` — it controls nothing and exists only to satisfy the `TransactionV2` protocol requirement).

## Manual Start

### 1. Database (PostgreSQL)

```bash
docker compose up -d
```

Starts PostgreSQL 17 on port 5432. Credentials: `postgres/postgres`, database: `multisig_orchestrator`.

### 2. Backend (Rust/Axum)

```bash
cd multisig-server
cp .env.example .env   # first time only
cargo run
```

Runs on http://localhost:3001. Auto-applies database migrations on startup.

### 3. Frontend (React/TanStack Start)

```bash
cd multisig-app
pnpm install             # first time only
pnpm run dev
```

Runs on http://localhost:3000.

## Development

This project uses [pnpm](https://pnpm.io/) as the package manager with a workspace setup.

```bash
pnpm install              # install all dependencies
pnpm fmt                  # format all files (oxfmt)
pnpm fmt:check            # check formatting without writing
pnpm lint                 # lint JS/TS files (oxlint)
```

Pre-commit hooks (via Husky + lint-staged) automatically format and lint staged files.

## Shutdown

```bash
docker compose down     # stop PostgreSQL
```

`./dev.sh` handles graceful shutdown of all services on Ctrl+C.
