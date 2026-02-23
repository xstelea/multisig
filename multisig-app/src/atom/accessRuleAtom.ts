import { Effect, Layer, Stream, Ref } from 'effect'
import { makeAtomRuntime } from './makeRuntimeAtom'
import {
  OrchestratorClient,
  OrchestratorClientLayer,
} from './orchestratorClient'
import { RadixDappToolkit } from '@/lib/dappToolkit'
import type { WalletData } from '@radixdlt/radix-dapp-toolkit'

const runtime = makeAtomRuntime(
  Layer.mergeAll(OrchestratorClientLayer, RadixDappToolkit.Live),
)

export const accessRuleAtom = runtime.atom(
  Effect.gen(function* () {
    const client = yield* OrchestratorClient
    return yield* client.getAccessRule()
  }),
)

export const walletDataAtom = runtime.atom(
  Effect.fnUntraced(function* (get) {
    const rdtRef = yield* RadixDappToolkit
    const rdt = yield* Ref.get(rdtRef)

    const walletData = Stream.asyncScoped<WalletData>((emit) =>
      Effect.gen(function* () {
        const subscription = rdt.walletApi.walletData$.subscribe((data) =>
          emit.single(data),
        )
        return Effect.sync(() => subscription.unsubscribe())
      }),
    )

    yield* Stream.runForEach(walletData, (value) =>
      Effect.sync(() => get.setSelf(Effect.succeed(value))),
    )

    return rdt.walletApi.getWalletData()
  }),
)

export const dappToolkitAtom = runtime.atom(
  Effect.gen(function* () {
    const rdtRef = yield* RadixDappToolkit
    return yield* Ref.get(rdtRef)
  }),
)
