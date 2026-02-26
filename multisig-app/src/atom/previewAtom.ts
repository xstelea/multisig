import { Effect, ConfigProvider, Layer } from "effect";
import { GatewayApiClient } from "@radix-effects/gateway";
import { makeAtomRuntime } from "./makeRuntimeAtom";
import { envVars } from "@/lib/envVars";

const GatewayLayer = GatewayApiClient.Default.pipe(
  Layer.provide(
    Layer.setConfigProvider(
      ConfigProvider.fromMap(
        new Map([["NETWORK_ID", String(envVars.NETWORK_ID)]])
      )
    )
  )
);

const runtime = makeAtomRuntime(GatewayLayer);

/** Strip trailing YIELD_TO_PARENT from a subintent manifest for preview. */
const stripYieldToParent = (manifest: string): string =>
  manifest.replace(/\s*YIELD_TO_PARENT\s*;\s*$/, "\n");

export const makePreviewTransactionAtom = () =>
  runtime.fn((manifestText: string) =>
    Effect.gen(function* () {
      const gateway = yield* GatewayApiClient;
      const manifest = stripYieldToParent(manifestText);

      const result = yield* gateway.transaction.innerClient.transactionPreview({
        transactionPreviewRequest: {
          manifest,
          start_epoch_inclusive: 1,
          end_epoch_exclusive: 2,
          nonce: 1,
          signer_public_keys: [],
          flags: {
            use_free_credit: true,
            assume_all_signature_proofs: true,
            skip_epoch_check: true,
          },
        },
      });

      return result;
    })
  );
