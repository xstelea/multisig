import {
  RadixDappToolkit as RadixDappToolkitFactory,
  DataRequestBuilder,
} from '@radixdlt/radix-dapp-toolkit'
import { Context, Effect, Layer, Ref } from 'effect'
import { envVars } from './envVars'

export class RadixDappToolkit extends Context.Tag('RadixDappToolkit')<
  RadixDappToolkit,
  Ref.Ref<RadixDappToolkitFactory>
>() {
  static Live = Layer.scoped(
    this,
    Effect.gen(function* () {
      if (typeof window === 'undefined') {
        return yield* Effect.fail(
          new Error('RadixDappToolkit requires browser environment'),
        )
      }

      const rdt = RadixDappToolkitFactory({
        networkId: envVars.NETWORK_ID,
        dAppDefinitionAddress: envVars.DAPP_DEFINITION_ADDRESS,
      })

      rdt.walletApi.setRequestData(DataRequestBuilder.accounts().atLeast(1))

      yield* Effect.addFinalizer(() => Effect.sync(() => rdt.destroy()))

      return yield* Ref.make(rdt)
    }),
  )
}
