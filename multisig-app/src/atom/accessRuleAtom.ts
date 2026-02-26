import { Effect, Layer, Ref } from "effect";
import { makeAtomRuntime } from "./makeRuntimeAtom";
import {
  OrchestratorClient,
  OrchestratorClientLayer,
} from "./orchestratorClient";
import { RadixDappToolkit } from "@/lib/dappToolkit";

const runtime = makeAtomRuntime(
  Layer.mergeAll(OrchestratorClientLayer, RadixDappToolkit.Live)
);

export const accessRuleAtom = runtime.atom(
  Effect.gen(function* () {
    const client = yield* OrchestratorClient;
    return yield* client.getAccessRule();
  })
);

export const dappToolkitAtom = runtime.atom(
  Effect.gen(function* () {
    const rdtRef = yield* RadixDappToolkit;
    return yield* Ref.get(rdtRef);
  })
);
