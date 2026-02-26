import { Effect, Ref } from "effect";
import { makeAtomRuntime } from "./makeRuntimeAtom";
import { RadixDappToolkit } from "@/lib/dappToolkit";

const runtime = makeAtomRuntime(RadixDappToolkit.Live);

export const dappToolkitAtom = runtime.atom(
  Effect.gen(function* () {
    const rdtRef = yield* RadixDappToolkit;
    return yield* Ref.get(rdtRef);
  })
);
