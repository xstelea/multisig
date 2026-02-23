import { Context, Effect, Layer, Schema } from "effect";
import {
  FetchHttpClient,
  HttpClient,
  HttpClientRequest,
} from "@effect/platform";
import { envVars } from "@/lib/envVars";

// --- Response schemas ---

export const SignerInfoSchema = Schema.Struct({
  key_hash: Schema.String,
  key_type: Schema.String,
  badge_resource: Schema.String,
  badge_local_id: Schema.String,
});
export type SignerInfo = typeof SignerInfoSchema.Type;

export const AccessRuleInfoSchema = Schema.Struct({
  signers: Schema.Array(SignerInfoSchema),
  threshold: Schema.Number,
});
export type AccessRuleInfo = typeof AccessRuleInfoSchema.Type;

export const ProposalSchema = Schema.Struct({
  id: Schema.String,
  manifest_text: Schema.String,
  treasury_account: Schema.NullOr(Schema.String),
  epoch_min: Schema.Number,
  epoch_max: Schema.Number,
  status: Schema.String,
  subintent_hash: Schema.NullOr(Schema.String),
  intent_discriminator: Schema.Number,
  created_at: Schema.String,
  submitted_at: Schema.NullOr(Schema.String),
  tx_id: Schema.NullOr(Schema.String),
});
export type Proposal = typeof ProposalSchema.Type;

export const SignerStatusSchema = Schema.Struct({
  key_hash: Schema.String,
  key_type: Schema.String,
  has_signed: Schema.Boolean,
});
export type SignerStatus = typeof SignerStatusSchema.Type;

export const SignatureSummarySchema = Schema.Struct({
  signer_public_key: Schema.String,
  signer_key_hash: Schema.String,
  created_at: Schema.String,
});
export type SignatureSummary = typeof SignatureSummarySchema.Type;

export const SignatureStatusSchema = Schema.Struct({
  proposal_id: Schema.String,
  signatures: Schema.Array(SignatureSummarySchema),
  threshold: Schema.Number,
  collected: Schema.Number,
  remaining: Schema.Number,
  signers: Schema.Array(SignerStatusSchema),
});
export type SignatureStatusType = typeof SignatureStatusSchema.Type;

// --- Service definition ---

export class OrchestratorClient extends Context.Tag("OrchestratorClient")<
  OrchestratorClient,
  {
    readonly health: () => Effect.Effect<{ status: string }, Error>;
    readonly getAccessRule: () => Effect.Effect<AccessRuleInfo, Error>;
    readonly createProposal: (input: {
      manifest_text: string;
      expiry_epoch: number;
    }) => Effect.Effect<Proposal, Error>;
    readonly listProposals: () => Effect.Effect<ReadonlyArray<Proposal>, Error>;
    readonly getProposal: (id: string) => Effect.Effect<Proposal, Error>;
    readonly signProposal: (
      id: string,
      signedPartialTransactionHex: string
    ) => Effect.Effect<SignatureStatusType, Error>;
    readonly getSignatureStatus: (
      id: string
    ) => Effect.Effect<SignatureStatusType, Error>;
  }
>() {}

// --- Live implementation ---

const OrchestratorClientLive = Layer.effect(
  OrchestratorClient,
  Effect.gen(function* () {
    const client = yield* HttpClient.HttpClient;
    const baseUrl = envVars.ORCHESTRATOR_URL;

    return {
      health: () =>
        client.execute(HttpClientRequest.get(`${baseUrl}/health`)).pipe(
          Effect.flatMap((res) => res.json),
          Effect.flatMap(
            Schema.decodeUnknown(Schema.Struct({ status: Schema.String }))
          ),
          Effect.scoped,
          Effect.catchAll((e) => Effect.fail(new Error(String(e))))
        ),

      getAccessRule: () =>
        client
          .execute(HttpClientRequest.get(`${baseUrl}/account/access-rule`))
          .pipe(
            Effect.flatMap((res) => res.json),
            Effect.flatMap(Schema.decodeUnknown(AccessRuleInfoSchema)),
            Effect.scoped,
            Effect.catchAll((e) => Effect.fail(new Error(String(e))))
          ),

      createProposal: (input: {
        manifest_text: string;
        expiry_epoch: number;
      }) =>
        HttpClientRequest.post(`${baseUrl}/proposals`).pipe(
          HttpClientRequest.bodyJson(input),
          Effect.flatMap((req) => client.execute(req)),
          Effect.flatMap((res) => res.json),
          Effect.flatMap(Schema.decodeUnknown(ProposalSchema)),
          Effect.scoped,
          Effect.catchAll((e) => Effect.fail(new Error(String(e))))
        ),

      listProposals: () =>
        client.execute(HttpClientRequest.get(`${baseUrl}/proposals`)).pipe(
          Effect.flatMap((res) => res.json),
          Effect.flatMap(Schema.decodeUnknown(Schema.Array(ProposalSchema))),
          Effect.scoped,
          Effect.catchAll((e) => Effect.fail(new Error(String(e))))
        ),

      getProposal: (id: string) =>
        client
          .execute(HttpClientRequest.get(`${baseUrl}/proposals/${id}`))
          .pipe(
            Effect.flatMap((res) => res.json),
            Effect.flatMap(Schema.decodeUnknown(ProposalSchema)),
            Effect.scoped,
            Effect.catchAll((e) => Effect.fail(new Error(String(e))))
          ),

      signProposal: (id: string, signedPartialTransactionHex: string) =>
        HttpClientRequest.post(`${baseUrl}/proposals/${id}/sign`).pipe(
          HttpClientRequest.bodyJson({
            signed_partial_transaction_hex: signedPartialTransactionHex,
          }),
          Effect.flatMap((req) => client.execute(req)),
          Effect.flatMap((res) => res.json),
          Effect.flatMap(Schema.decodeUnknown(SignatureStatusSchema)),
          Effect.scoped,
          Effect.catchAll((e) => Effect.fail(new Error(String(e))))
        ),

      getSignatureStatus: (id: string) =>
        client
          .execute(
            HttpClientRequest.get(`${baseUrl}/proposals/${id}/signatures`)
          )
          .pipe(
            Effect.flatMap((res) => res.json),
            Effect.flatMap(Schema.decodeUnknown(SignatureStatusSchema)),
            Effect.scoped,
            Effect.catchAll((e) => Effect.fail(new Error(String(e))))
          ),
    };
  })
);

export const OrchestratorClientLayer = OrchestratorClientLive.pipe(
  Layer.provide(FetchHttpClient.layer)
);
