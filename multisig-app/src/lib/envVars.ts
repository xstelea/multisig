import * as Schema from 'effect/Schema'
import * as Either from 'effect/Either'
import { pipe } from 'effect'
import { TreeFormatter } from 'effect/ParseResult'

class EnvVars extends Schema.Class<EnvVars>('EnvVars')({
  NETWORK_ID: Schema.NumberFromString,
  DAPP_DEFINITION_ADDRESS: Schema.String,
  ORCHESTRATOR_URL: Schema.String,
}) {}

export const envVars = pipe(
  {
    NETWORK_ID: import.meta.env.VITE_PUBLIC_NETWORK_ID as unknown,
    DAPP_DEFINITION_ADDRESS: import.meta.env
      .VITE_PUBLIC_DAPP_DEFINITION_ADDRESS as unknown,
    ORCHESTRATOR_URL: import.meta.env.VITE_ORCHESTRATOR_URL as unknown,
  },
  Schema.decodeUnknownEither(EnvVars),
  Either.getOrElse((parseIssue) => {
    throw new Error(
      `Invalid environment variables: ${TreeFormatter.formatErrorSync(parseIssue)}`,
    )
  }),
)
