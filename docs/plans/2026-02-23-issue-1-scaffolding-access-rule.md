# Issue 1: Project Scaffolding + Access Rule Display

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Stand up full stack (Rust/axum backend + TanStack Start frontend) end-to-end and display the multisig account's current access rule.

**Architecture:** Rust backend serves REST API on configurable port, reads access rule from Stokenet Gateway API, connects to PostgreSQL. TanStack Start frontend consumes API via Effect service, renders access rule + Radix wallet connect button.

**Tech Stack:** Rust (axum, sqlx, reqwest, serde), TanStack Start (React 19, Effect, @effect-atom/atom-react, @radixdlt/radix-dapp-toolkit, Tailwind CSS v4)

**Test multisig account (created via POC):** `account_tdx_2_1cx3u3xgr9anc9fk54dxzsz6k2n6lnadludkx4mx5re5erl8jt9lpnp`

---

### Task 1: Backend crate scaffolding

**Files:**
- Create: `multisig-server/Cargo.toml`
- Create: `multisig-server/src/main.rs`
- Create: `multisig-server/.env.example`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "multisig-server"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = { version = "0.8", features = ["json"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
reqwest = { version = "0.12", features = ["json"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "postgres", "uuid", "chrono"] }
tower-http = { version = "0.6", features = ["cors"] }
anyhow = "1.0"
thiserror = "2.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
dotenvy = "0.15"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
```

**Step 2: Create minimal main.rs**

Axum app with `/health` returning 200. Read env vars for PORT, DATABASE_URL, GATEWAY_URL, MULTISIG_ACCOUNT_ADDRESS, FRONTEND_ORIGIN. Set up tracing. PostgreSQL pool via sqlx. CORS middleware.

**Step 3: Create .env.example**

```
PORT=3001
DATABASE_URL=postgres://localhost/multisig_orchestrator
GATEWAY_URL=https://babylon-stokenet-gateway.radixdlt.com
MULTISIG_ACCOUNT_ADDRESS=account_tdx_2_1cx3u3xgr9anc9fk54dxzsz6k2n6lnadludkx4mx5re5erl8jt9lpnp
FRONTEND_ORIGIN=http://localhost:3000
```

**Step 4: Verify it compiles and /health returns 200**

Run: `cargo build` then `cargo run` and `curl localhost:3001/health`

**Step 5: Commit**

---

### Task 2: PostgreSQL migrations setup

**Files:**
- Create: `multisig-server/migrations/001_initial.sql`

**Step 1: Create empty initial migration**

```sql
-- Initial migration: empty schema placeholder
SELECT 1;
```

**Step 2: Run migrations on startup in main.rs**

Add `sqlx::migrate!("./migrations").run(&pool).await` to startup.

**Step 3: Verify** — app starts, runs migration, /health still 200.

**Step 4: Commit**

---

### Task 3: GatewayClient — read_access_rule

**Files:**
- Create: `multisig-server/src/gateway.rs`

**Step 1: Write unit test for access rule parsing**

Test parsing real Gateway API JSON response (from our test account). The response structure:

```json
{
  "owner": {
    "rule": {
      "type": "Protected",
      "access_rule": {
        "type": "ProofRule",
        "proof_rule": {
          "type": "CountOf",
          "count": 3,
          "list": [
            {
              "type": "NonFungible",
              "non_fungible": {
                "local_id": { "simple_rep": "[hash_hex]" },
                "resource_address": "resource_tdx_2_1nfxxxxxxxxxxed25sgxxxxxxxxx..."
              }
            }
          ]
        }
      }
    }
  }
}
```

Parse into `AccessRuleInfo { signers: Vec<SignerInfo>, threshold: u8 }` where `SignerInfo { key_hash: String, key_type: String, badge_resource: String, badge_local_id: String }`.

**Step 2: Implement GatewayClient struct**

- `new(base_url: String)` — reqwest::Client + base_url
- `read_access_rule(account_address: &str) -> Result<AccessRuleInfo>` — POST /state/entity/details, parse role_assignments.owner.rule
- `get_current_epoch() -> Result<u64>` — reuse POC's /status/gateway-status pattern

**Step 3: Run unit tests**

**Step 4: Write `#[ignore]` integration test against Stokenet**

Test against `account_tdx_2_1cx3u3xgr9anc9fk54dxzsz6k2n6lnadludkx4mx5re5erl8jt9lpnp` — expect threshold=3, 4 signers.

**Step 5: Commit**

---

### Task 4: Access rule API endpoint

**Files:**
- Modify: `multisig-server/src/main.rs`

**Step 1: Add GET /account/access-rule endpoint**

Returns JSON: `{ signers: [{ key_hash, key_type, badge_resource, badge_local_id }], threshold: number }`

Uses shared AppState (GatewayClient + config + pool).

**Step 2: Verify end-to-end**

Run server, curl endpoint, see real access rule data from Stokenet.

**Step 3: Commit**

---

### Task 5: Frontend scaffolding

**Files:**
- Create: `multisig-app/package.json`
- Create: `multisig-app/vite.config.ts`
- Create: `multisig-app/tsconfig.json`
- Create: `multisig-app/src/routes/__root.tsx`
- Create: `multisig-app/src/routes/index.tsx`
- Create: `multisig-app/src/router.tsx`
- Create: `multisig-app/src/app.css`
- Create: `multisig-app/.env`

Pattern: Follow consultation_v2 app structure exactly.

**Step 1: Create package.json with deps**

Key deps: @tanstack/react-start, @tanstack/react-router, react 19, effect, @effect-atom/atom-react, @radixdlt/radix-dapp-toolkit, tailwindcss v4, sonner.

**Step 2: Create vite.config.ts**

tanstackStart() before viteReact(), tailwindcss plugin, viteTsConfigPaths.

**Step 3: Create tsconfig.json**

Target ES2022, bundler moduleResolution, `@/*` path alias.

**Step 4: Create root layout + index route**

Root layout: RegistryProvider wrapper, Outlet, minimal shell.
Index route: placeholder "Multisig Orchestrator" heading.

**Step 5: Verify** — `pnpm install && pnpm dev` loads the app.

**Step 6: Commit**

---

### Task 6: Effect runtime + Radix dApp Toolkit + OrchestratorClient

**Files:**
- Create: `multisig-app/src/lib/envVars.ts`
- Create: `multisig-app/src/lib/dappToolkit.ts`
- Create: `multisig-app/src/atom/makeRuntimeAtom.ts`
- Create: `multisig-app/src/atom/orchestratorClient.ts`
- Create: `multisig-app/src/atom/accessRuleAtom.ts`

**Step 1: Create envVars.ts**

Schema-validated env vars: VITE_PUBLIC_NETWORK_ID, VITE_PUBLIC_DAPP_DEFINITION_ADDRESS, VITE_ORCHESTRATOR_URL.

**Step 2: Create dappToolkit.ts**

RadixDappToolkit Effect service with Live layer (from consultation_v2 pattern).

**Step 3: Create makeRuntimeAtom.ts**

Atom.context with defaultMemoMap, global logger layer.

**Step 4: Create orchestratorClient.ts**

Effect service wrapping backend API: `health()` and `getAccessRule()` methods.
Uses FetchHttpClient, returns typed responses via Schema.

**Step 5: Create accessRuleAtom.ts**

Atom that fetches access rule from OrchestratorClient on mount.

**Step 6: Commit**

---

### Task 7: Home page — display access rule + wallet connect

**Files:**
- Modify: `multisig-app/src/routes/index.tsx`
- Modify: `multisig-app/src/routes/__root.tsx`

**Step 1: Add connect button to root layout**

Import RadixDappToolkit, mount via useAtomMount, render `<radix-connect-button>`.

**Step 2: Build home page**

Display:
- Connected wallet state (from walletDataAtom)
- Multisig account address
- Current signers with key hashes
- Required threshold (e.g. "3 of 4 signatures required")

Uses Result.builder pattern for loading/error/success states.

**Step 3: Verify end-to-end** — backend running + frontend running, page shows real access rule.

**Step 4: Commit**

---

## Summary

| Task | Description |
|------|-------------|
| 1 | Backend crate scaffolding (axum, config, /health) |
| 2 | PostgreSQL migrations setup |
| 3 | GatewayClient — read_access_rule with tests |
| 4 | Access rule API endpoint |
| 5 | Frontend scaffolding (TanStack Start + Tailwind) |
| 6 | Effect runtime + RDT + OrchestratorClient |
| 7 | Home page — display access rule + wallet connect |
