# Spec Index

Quick navigation for the 15 spec files in `context/`.

## Effect (8 specs)

| File                                               | Description                                                                             | Key Types / APIs                                                                                                                          |
| -------------------------------------------------- | --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| [`effect-atom.md`](context/effect-atom.md)         | Reactive state management for Effect + React (atoms, subscriptions, Result pattern)     | `Atom.make`, `Atom.family`, `Atom.context`, `runtime.atom`, `runtime.fn`, `useAtomValue`, `useAtomSet`, `Result<A, E>`                    |
| [`effect-Context.md`](context/effect-Context.md)   | Dependency injection foundation: `Context.Tag`, service composition, `R` type parameter | `Context.Tag`, `Effect.Service`, `Effect.provideService`, `Layer.succeed`, `Layer.effect`, `Layer.scoped`                                 |
| [`effect-Layer.md`](context/effect-Layer.md)       | Composable memoized blueprints for service dependency graphs (`Layer<ROut, E, RIn>`)    | `Layer.succeed`, `Layer.effect`, `Layer.scoped`, `Layer.merge`, `Layer.mergeAll`, `Layer.provide`, `Layer.provideMerge`, `ManagedRuntime` |
| [`effect-Pipe.md`](context/effect-Pipe.md)         | `pipe()` function and `.pipe()` method for top-to-bottom composition                    | `pipe()`, `.pipe()`, `flow()`                                                                                                             |
| [`effect-Queue.md`](context/effect-Queue.md)       | Fiber-safe async bounded queues for producer/consumer concurrency                       | `Queue.bounded`, `Queue.unbounded`, `Queue.sliding`, `Queue.dropping`, `Queue.offer`, `Queue.take`                                        |
| [`effect-Rpc.md`](context/effect-Rpc.md)           | Type-safe transport-agnostic RPC (WebSocket, HTTP, Socket, Worker, Stdio)               | `Rpc.make`, `Rpc.group`, `RpcRouter`, `RpcClient`, `RpcSerialization`, `Rpc.stream`                                                       |
| [`effect-Schema.md`](context/effect-Schema.md)     | Runtime validation and transformation with full TypeScript type inference               | `Schema.Struct`, `Schema.Class`, `Schema.decode`, `Schema.encode`, `Schema.brand`, `Schema.transform`, `Schema.filter`                    |
| [`effect-Platform.md`](context/effect-Platform.md) | Platform-independent abstractions: HTTP client/server, filesystem, terminal, workers    | `HttpClient`, `HttpServer`, `FetchHttpClient`, `HttpRouter`, `FileSystem`, `Terminal`, `Worker`                                           |

## Radix (5 specs)

| File                                                         | Description                                                                            | Key Types / APIs                                                                                                             |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| [`radix-SubIntents.md`](context/radix-SubIntents.md)         | Subintents/pre-authorizations: composable atomic multi-party transactions              | `SubintentManifest`, `PartialTransaction`, `sendPreAuthorizationRequest`, `expireAtTime`, `YIELD_TO_PARENT`                  |
| [`radix-transactions.md`](context/radix-transactions.md)     | `radix-transactions` Rust crate: build, sign, validate, serialize transactions (V1+V2) | `TransactionV2Builder`, `PartialTransactionV2Builder`, `SubintentManifestV2`, `NotarizedTransactionV2`, `IntentSignaturesV2` |
| [`radix-GatewayRustSdk.md`](context/radix-GatewayRustSdk.md) | `radix-client` Rust crate: typed async/blocking HTTP clients for Gateway + Core APIs   | `GatewayClientAsync`, `GatewayClientBlocking`, `TransactionApi`, `StateApi`, `StatusApi`                                     |
| [`radix-Sbor.md`](context/radix-Sbor.md)                     | SBOR binary serialization format (wire format, derive macros, schema system)           | `ScryptoSbor`, `ManifestSbor`, `sbor_decode`, `sbor_encode`, `manifest_encode`, `ScryptoValue`                               |
| [`radix-Gateway.md`](context/radix-Gateway.md)               | `@radix-effects/gateway`: Effect wrapper around Radix Gateway API with tagged errors   | `GatewayApiClient`, `GatewayApiClientLayer`, `TransactionService`, `StateService`, `GatewayError`                            |

## Frontend (1 spec)

| File                                               | Description                                                                | Key Types / APIs                                                                                                                                |
| -------------------------------------------------- | -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| [`tanstack-Router.md`](context/tanstack-Router.md) | TanStack Router: type-safe file-based routing, search params, loaders, SSR | `createRouter`, `createRoute`, `createFileRoute`, `RouterState`, `RouteMatch`, `Link`, `useNavigate`, `useSearch`, `useParams`, `useLoaderData` |

## Application (1 spec)

| File                                         | Description                                                                  | Key Types / APIs                                                                                                                            |
| -------------------------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| [`consultation.md`](context/consultation.md) | Consultation dApp architecture: governance voting on Radix with Effect Atoms | `makeAtomRuntime`, `Atom.context`, `RadixDappToolkit`, `SendTransaction`, `GovernanceComponent`, `VoteClient`, `useAtomValue`, `useAtomSet` |

## Cross-Reference Map

```
                        ┌─────────────────┐
                        │   effect-Pipe    │ (used everywhere)
                        └─────────────────┘

  ┌──────────────┐         ┌──────────────┐         ┌───────────────┐
  │ effect-Context│◄───────►│ effect-Layer  │◄───────►│effect-Platform│
  └──────┬───────┘         └──────┬───────┘         └───────┬───────┘
         │                        │                         │
         ▼                        │                         ▼
  ┌──────────────┐               │                  ┌──────────────┐
  │  effect-atom  │               │                  │  effect-Rpc   │
  └──────┬───────┘               │                  └──────┬───────┘
         │                        │                    ▲    │
         │                        │                    │    ▼
         │                        │              ┌─────┴────────┐
         │                        │              │ effect-Schema │
         │                        │              └──────┬───────┘
         │                        │                     │
         │                        ▼                     ▼
         │              ┌──────────────────┐   ┌──────────────┐
         │              │  radix-Gateway    │◄──┤ effect-Queue  │
         │              └────────┬─────────┘   └──────────────┘
         │                       │
         ▼                       ▼
  ┌──────────────────────────────────────────┐
  │            consultation dApp              │
  │  (effect-atom + radix-Gateway + Router)   │
  └──────────────────┬───────────────────────┘
                     │
                     ▼
            ┌─────────────────┐
            │ tanstack-Router  │
            └─────────────────┘

  ┌──────────────────┐   ┌──────────────┐   ┌───────────────────┐
  │radix-SubIntents   │◄─►│radix-txns    │◄─►│  radix-Sbor        │
  └──────────────────┘   └──────┬───────┘   └────────┬──────────┘
                                │                     │
                                ▼                     ▼
                       ┌─────────────────┐   ┌──────────────────┐
                       │radix-GatewayRust│   │  radix-Gateway    │
                       └─────────────────┘   └──────────────────┘
```

**Key relationships:**

| Link                                  | Relationship                                       |
| ------------------------------------- | -------------------------------------------------- |
| effect-Context ↔ effect-Layer         | Context defines services, Layer composes them      |
| effect-Schema ↔ effect-Rpc            | RPC procedures defined with Schema                 |
| effect-Platform ↔ effect-Rpc          | RPC serves over Platform HTTP/WebSocket/Socket     |
| effect-Queue ↔ effect-Rpc             | Streaming RPC backed by Queue                      |
| effect-Layer ↔ effect-Platform        | Platform provides service Layers                   |
| effect-Layer ↔ radix-Gateway          | Gateway uses Layer composition + memoization       |
| effect-Schema ↔ radix-Gateway         | Gateway decodes SBOR types via Schema              |
| effect-atom ↔ effect-Context          | Atoms use Effect context for dependencies          |
| effect-Pipe                           | Universal composition primitive (used everywhere)  |
| radix-SubIntents ↔ radix-transactions | Subintents are V2 transaction model primitives     |
| radix-transactions ↔ radix-Sbor       | Transactions serialize via SBOR                    |
| radix-Sbor ↔ radix-Gateway            | Gateway decodes SBOR payloads (sbor-ez-mode)       |
| radix-GatewayRustSdk → radix-Gateway  | Rust SDK vs Effect wrapper for same API surface    |
| consultation → effect-atom            | App state via Effect Atoms + dual runtime          |
| consultation → radix-Gateway          | Blockchain reads via Effect Gateway wrapper        |
| consultation → tanstack-Router        | File-based routing with search param validation    |
| tanstack-Router ↔ effect-Schema       | Search params validated via Effect Schema adapters |

## Quick Lookup

| I need to...                           | Read                                                                                                            |
| -------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Define typed RPC endpoints             | [`effect-Rpc.md`](context/effect-Rpc.md)                                                                        |
| Build HTTP client or server            | [`effect-Platform.md`](context/effect-Platform.md)                                                              |
| Validate data / define schemas         | [`effect-Schema.md`](context/effect-Schema.md)                                                                  |
| Wire up dependency injection           | [`effect-Context.md`](context/effect-Context.md) then [`effect-Layer.md`](context/effect-Layer.md)              |
| Manage reactive UI state               | [`effect-atom.md`](context/effect-atom.md)                                                                      |
| Do concurrent producer/consumer        | [`effect-Queue.md`](context/effect-Queue.md)                                                                    |
| Compose function pipelines             | [`effect-Pipe.md`](context/effect-Pipe.md)                                                                      |
| Build or sign a transaction            | [`radix-transactions.md`](context/radix-transactions.md)                                                        |
| Use subintents / pre-auth              | [`radix-SubIntents.md`](context/radix-SubIntents.md)                                                            |
| Query Gateway API (Rust)               | [`radix-GatewayRustSdk.md`](context/radix-GatewayRustSdk.md)                                                    |
| Query Gateway API (Effect/TS)          | [`radix-Gateway.md`](context/radix-Gateway.md)                                                                  |
| Encode/decode SBOR binary              | [`radix-Sbor.md`](context/radix-Sbor.md)                                                                        |
| Set up file-based routing              | [`tanstack-Router.md`](context/tanstack-Router.md)                                                              |
| Validate search params in routes       | [`tanstack-Router.md`](context/tanstack-Router.md) + [`effect-Schema.md`](context/effect-Schema.md)             |
| See the consultation dApp architecture | [`consultation.md`](context/consultation.md)                                                                    |
| Build atoms with Effect DI             | [`effect-atom.md`](context/effect-atom.md) + [`consultation.md`](context/consultation.md)                       |
| Build multi-party transactions         | [`radix-SubIntents.md`](context/radix-SubIntents.md) + [`radix-transactions.md`](context/radix-transactions.md) |
| Understand the full app stack          | [`consultation.md`](context/consultation.md) (integrates all layers)                                            |
