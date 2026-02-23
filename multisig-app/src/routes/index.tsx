import { createFileRoute } from '@tanstack/react-router'
import { Result, useAtomValue, useAtomMount } from '@effect-atom/atom-react'
import {
  accessRuleAtom,
  walletDataAtom,
  dappToolkitAtom,
} from '@/atom/accessRuleAtom'
import { envVars } from '@/lib/envVars'
import type { SignerInfo } from '@/atom/orchestratorClient'

export const Route = createFileRoute('/')({
  component: HomePage,
})

function HomePage() {
  useAtomMount(dappToolkitAtom)

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">
          Multisig Dashboard
        </h1>
        <p className="text-muted-foreground mt-1">
          View the multisig account's access rule and signer configuration.
        </p>
      </div>

      <WalletStatus />
      <AccessRuleDisplay />
    </div>
  )
}

function WalletStatus() {
  const walletResult = useAtomValue(walletDataAtom)

  return (
    <section className="border border-border rounded-lg p-6 bg-card">
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider mb-3">
        Connected Wallet
      </h2>
      {Result.builder(walletResult)
        .onInitial(() => (
          <p className="text-muted-foreground">Connecting to wallet...</p>
        ))
        .onSuccess((walletData) => {
          if (!walletData) return null
          const accounts = walletData.accounts ?? []
          if (accounts.length === 0) {
            return (
              <p className="text-muted-foreground">
                No accounts connected. Click the connect button above.
              </p>
            )
          }
          return (
            <div className="space-y-2">
              {accounts.map((account) => (
                <div
                  key={account.address}
                  className="flex items-center gap-3 text-sm"
                >
                  <span className="font-medium">{account.label}</span>
                  <code className="text-xs text-muted-foreground font-mono">
                    {account.address.slice(0, 20)}...
                    {account.address.slice(-8)}
                  </code>
                </div>
              ))}
            </div>
          )
        })
        .onFailure(() => (
          <p className="text-muted-foreground">
            Wallet not available. Install the Radix Wallet extension.
          </p>
        ))
        .render()}
    </section>
  )
}

function AccessRuleDisplay() {
  const accessRuleResult = useAtomValue(accessRuleAtom)

  return (
    <section className="border border-border rounded-lg p-6 bg-card">
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider mb-1">
        Multisig Access Rule
      </h2>
      <p className="text-xs text-muted-foreground mb-4 font-mono">
        {envVars.DAPP_DEFINITION_ADDRESS
          ? `Account: ${envVars.DAPP_DEFINITION_ADDRESS.slice(0, 20)}...`
          : 'Account not configured'}
      </p>

      {Result.builder(accessRuleResult)
        .onInitial(() => (
          <div className="space-y-3">
            <div className="h-6 w-48 bg-muted rounded animate-pulse" />
            <div className="h-4 w-full bg-muted rounded animate-pulse" />
            <div className="h-4 w-full bg-muted rounded animate-pulse" />
            <div className="h-4 w-full bg-muted rounded animate-pulse" />
          </div>
        ))
        .onSuccess((accessRule) => (
          <div className="space-y-4">
            <div className="flex items-baseline gap-2">
              <span className="text-3xl font-bold">{accessRule.threshold}</span>
              <span className="text-muted-foreground">
                of {accessRule.signers.length} signatures required
              </span>
            </div>

            <div className="space-y-2">
              <h3 className="text-sm font-medium text-muted-foreground">
                Signers
              </h3>
              <div className="divide-y divide-border">
                {accessRule.signers.map((signer, i) => (
                  <SignerRow key={signer.key_hash} signer={signer} index={i} />
                ))}
              </div>
            </div>
          </div>
        ))
        .onFailure((error) => (
          <div className="text-red-400">
            <p>Failed to load access rule.</p>
            <p className="text-xs mt-1 text-muted-foreground">
              {String(error)}
            </p>
            <p className="text-xs mt-2 text-muted-foreground">
              Make sure the backend is running on {envVars.ORCHESTRATOR_URL}
            </p>
          </div>
        ))
        .render()}
    </section>
  )
}

function SignerRow({
  signer,
  index,
}: {
  signer: SignerInfo
  index: number
}) {
  return (
    <div className="py-3 flex items-center justify-between">
      <div className="flex items-center gap-3">
        <span className="text-xs text-muted-foreground w-6">#{index + 1}</span>
        <div>
          <code className="text-sm font-mono">
            {signer.key_hash.slice(0, 12)}...{signer.key_hash.slice(-8)}
          </code>
          <p className="text-xs text-muted-foreground mt-0.5">
            {signer.key_type}
          </p>
        </div>
      </div>
      <code className="text-xs text-muted-foreground font-mono">
        {signer.badge_local_id}
      </code>
    </div>
  )
}
