import { Effect } from "effect";
import { makeAtomRuntime } from "./makeRuntimeAtom";
import {
  OrchestratorClient,
  OrchestratorClientLayer,
} from "./orchestratorClient";

const runtime = makeAtomRuntime(OrchestratorClientLayer);

export const proposalListAtom = runtime.atom(
  Effect.gen(function* () {
    const client = yield* OrchestratorClient;
    return yield* client.listProposals();
  })
);

export const createProposalAtom = runtime.fn(
  (input: { manifest_text: string; expiry_epoch: number }, get) =>
    Effect.gen(function* () {
      const client = yield* OrchestratorClient;
      const proposal = yield* client.createProposal(input);
      get.refresh(proposalListAtom);
      return proposal;
    })
);

export const makeProposalDetailAtom = (id: string) =>
  runtime.atom(
    Effect.gen(function* () {
      const client = yield* OrchestratorClient;
      return yield* client.getProposal(id);
    })
  );
