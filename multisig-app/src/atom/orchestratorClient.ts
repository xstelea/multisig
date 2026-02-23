import { Context, Effect, Layer, Schema } from 'effect'
import { FetchHttpClient, HttpClient, HttpClientRequest } from '@effect/platform'
import { envVars } from '@/lib/envVars'

// --- Response schemas ---

export const SignerInfoSchema = Schema.Struct({
  key_hash: Schema.String,
  key_type: Schema.String,
  badge_resource: Schema.String,
  badge_local_id: Schema.String,
})
export type SignerInfo = typeof SignerInfoSchema.Type

export const AccessRuleInfoSchema = Schema.Struct({
  signers: Schema.Array(SignerInfoSchema),
  threshold: Schema.Number,
})
export type AccessRuleInfo = typeof AccessRuleInfoSchema.Type

// --- Service definition ---

export class OrchestratorClient extends Context.Tag('OrchestratorClient')<
  OrchestratorClient,
  {
    readonly health: () => Effect.Effect<{ status: string }, Error>
    readonly getAccessRule: () => Effect.Effect<AccessRuleInfo, Error>
  }
>() {}

// --- Live implementation ---

const OrchestratorClientLive = Layer.effect(
  OrchestratorClient,
  Effect.gen(function* () {
    const client = yield* HttpClient.HttpClient
    const baseUrl = envVars.ORCHESTRATOR_URL

    return {
      health: () =>
        client
          .execute(HttpClientRequest.get(`${baseUrl}/health`))
          .pipe(
            Effect.flatMap((res) => res.json),
            Effect.flatMap(Schema.decodeUnknown(Schema.Struct({ status: Schema.String }))),
            Effect.scoped,
            Effect.catchAll((e) => Effect.fail(new Error(String(e)))),
          ),

      getAccessRule: () =>
        client
          .execute(HttpClientRequest.get(`${baseUrl}/account/access-rule`))
          .pipe(
            Effect.flatMap((res) => res.json),
            Effect.flatMap(Schema.decodeUnknown(AccessRuleInfoSchema)),
            Effect.scoped,
            Effect.catchAll((e) => Effect.fail(new Error(String(e)))),
          ),
    }
  }),
)

export const OrchestratorClientLayer = OrchestratorClientLive.pipe(
  Layer.provide(FetchHttpClient.layer),
)
