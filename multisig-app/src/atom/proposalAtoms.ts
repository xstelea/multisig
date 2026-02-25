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

export const makeSignatureStatusAtom = (id: string) =>
  runtime.atom(
    Effect.gen(function* () {
      const client = yield* OrchestratorClient;
      return yield* client.getSignatureStatus(id);
    })
  );

export const makeSignProposalAtom = (
  proposalDetailAtom: ReturnType<typeof makeProposalDetailAtom>,
  signatureStatusAtom: ReturnType<typeof makeSignatureStatusAtom>
) =>
  runtime.fn(
    (input: { proposalId: string; signedPartialTransactionHex: string }, get) =>
      Effect.gen(function* () {
        const client = yield* OrchestratorClient;
        const result = yield* client.signProposal(
          input.proposalId,
          input.signedPartialTransactionHex
        );
        get.refresh(proposalDetailAtom);
        get.refresh(signatureStatusAtom);
        get.refresh(proposalListAtom);
        return result;
      })
  );

export const makeSubmitProposalAtom = (
  proposalDetailAtom: ReturnType<typeof makeProposalDetailAtom>
) =>
  runtime.fn((proposalId: string, get) =>
    Effect.gen(function* () {
      const client = yield* OrchestratorClient;
      const result = yield* client.submitProposal(proposalId);
      get.refresh(proposalDetailAtom);
      get.refresh(proposalListAtom);
      return result;
    })
  );
