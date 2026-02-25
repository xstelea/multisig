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

export const currentEpochAtom = runtime.atom(
  Effect.gen(function* () {
    const gateway = yield* GatewayApiClient;
    const status = yield* gateway.status.getCurrent();
    return status.ledger_state.epoch;
  })
);

export const epochDurationAtom = runtime.atom(
  Effect.gen(function* () {
    const gateway = yield* GatewayApiClient;

    const status = yield* gateway.status.getCurrent();
    const currentEpoch = status.ledger_state.epoch;

    const response = yield* gateway.stream.innerClient.streamTransactions({
      streamTransactionsRequest: {
        kind_filter: "EpochChange",
        order: "Desc",
        limit_per_page: 10,
      },
    });

    const items = response.items;
    if (items.length < 2) {
      return { currentEpoch, secondsPerEpoch: 300 };
    }

    let totalSeconds = 0;
    let pairs = 0;
    for (let i = 0; i < items.length - 1; i++) {
      const newer = new Date(items[i].round_timestamp).getTime();
      const older = new Date(items[i + 1].round_timestamp).getTime();
      const diff = (newer - older) / 1000;
      if (diff > 0) {
        totalSeconds += diff;
        pairs++;
      }
    }

    const secondsPerEpoch = pairs > 0 ? totalSeconds / pairs : 300;

    return { currentEpoch, secondsPerEpoch };
  })
);
