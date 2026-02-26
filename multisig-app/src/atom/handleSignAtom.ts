import { Effect, Layer, Ref } from "effect";
import { SubintentRequestBuilder } from "@radixdlt/radix-dapp-toolkit";
import { makeAtomRuntime } from "./makeRuntimeAtom";
import {
  OrchestratorClient,
  OrchestratorClientLayer,
} from "./orchestratorClient";
import type { Proposal } from "./orchestratorClient";
import { RadixDappToolkit } from "@/lib/dappToolkit";
import {
  proposalListAtom,
  makeProposalDetailAtom,
  makeSignatureStatusAtom,
} from "./proposalAtoms";

const runtime = makeAtomRuntime(
  Layer.mergeAll(OrchestratorClientLayer, RadixDappToolkit.Live)
);

export const makeHandleSignAtom = (
  proposalDetailAtom: ReturnType<typeof makeProposalDetailAtom>,
  signatureStatusAtom: ReturnType<typeof makeSignatureStatusAtom>
) =>
  runtime.fn((proposal: Proposal, get) =>
    Effect.gen(function* () {
      // Get the RDT instance from the service
      const rdtRef = yield* RadixDappToolkit;
      const rdt = yield* Ref.get(rdtRef);

      // Build manifest with YIELD_TO_PARENT if not present
      const subintentManifest = proposal.manifest_text.includes(
        "YIELD_TO_PARENT"
      )
        ? proposal.manifest_text
        : `${proposal.manifest_text.trimEnd()}\nYIELD_TO_PARENT;\n`;

      const header = {
        startEpochInclusive: proposal.epoch_min,
        endEpochExclusive: proposal.epoch_max,
        intentDiscriminator: proposal.intent_discriminator,
        minProposerTimestampInclusive: proposal.min_proposer_timestamp,
        maxProposerTimestampExclusive: proposal.max_proposer_timestamp,
      };

      // Send pre-authorization request to wallet
      const result = yield* Effect.tryPromise(() =>
        rdt.walletApi.sendPreAuthorizationRequest(
          SubintentRequestBuilder()
            .manifest(subintentManifest)
            .header(header)
            .setExpiration("atTime", proposal.max_proposer_timestamp)
        )
      );

      if (result.isErr()) {
        return yield* Effect.fail(
          new Error(
            `Wallet error: ${result.error.error} — ${result.error.message ?? "Unknown error"}`
          )
        );
      }

      const { signedPartialTransaction, subintentHash } = result.value;

      // Validate the wallet signed over the correct subintent
      if (
        proposal.subintent_hash &&
        subintentHash &&
        subintentHash !== proposal.subintent_hash
      ) {
        return yield* Effect.fail(
          new Error(
            "Your wallet produced a different subintent hash than expected. " +
              "It may not support custom subintent headers — please update " +
              "your Radix Wallet to the latest version."
          )
        );
      }

      // Send signed partial to backend
      const client = yield* OrchestratorClient;
      const signResult = yield* client.signProposal(
        proposal.id,
        signedPartialTransaction
      );

      get.refresh(proposalDetailAtom);
      get.refresh(signatureStatusAtom);
      get.refresh(proposalListAtom);

      return signResult;
    })
  );
