import { createFileRoute, Link } from "@tanstack/react-router";
import { Result, useAtomValue, useAtomMount } from "@effect-atom/atom-react";
import {
  accessRuleAtom,
  walletDataAtom,
  dappToolkitAtom,
} from "@/atom/accessRuleAtom";
import { proposalListAtom } from "@/atom/proposalAtoms";
import { epochDurationAtom } from "@/atom/gatewayAtoms";
import { formatEpochDelta } from "@/lib/epochTime";
import { envVars, dashboardBaseUrl } from "@/lib/envVars";
import { ClientOnly } from "@/lib/ClientOnly";
import type { Proposal, SignerInfo } from "@/atom/orchestratorClient";

export const Route = createFileRoute("/")({
  component: HomePage,
});

function HomePage() {
  return (
    <div className="space-y-8">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">
            Multisig Dashboard
          </h1>
          <p className="text-muted-foreground mt-1">
            Manage proposals and view the multisig account configuration.
          </p>
        </div>
        <Link
          to="/proposals/new"
          className="inline-flex items-center gap-2 rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/80 transition-colors"
        >
          New Proposal
        </Link>
      </div>

      <ClientOnly
        fallback={
          <>
            <WalletStatusSkeleton />
            <ProposalListSkeleton />
            <AccessRuleSkeleton />
          </>
        }
      >
        {() => <HomePageClient />}
      </ClientOnly>
    </div>
  );
}

function HomePageClient() {
  useAtomMount(dappToolkitAtom);
  return (
    <>
      <WalletStatus />
      <ProposalList />
      <AccessRuleDisplay />
    </>
  );
}

// --- Proposal List ---

function ProposalListSkeleton() {
  return (
    <section className="border border-border rounded-lg p-6 bg-card">
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider mb-3">
        Proposals
      </h2>
      <div className="space-y-3">
        <div className="h-12 w-full bg-muted rounded animate-pulse" />
        <div className="h-12 w-full bg-muted rounded animate-pulse" />
      </div>
    </section>
  );
}

const STATUS_COLORS: Record<string, string> = {
  created: "bg-blue-500/20 text-blue-400",
  signing: "bg-yellow-500/20 text-yellow-400",
  ready: "bg-green-500/20 text-green-400",
  submitting: "bg-purple-500/20 text-purple-400",
  committed: "bg-emerald-500/20 text-emerald-400",
  failed: "bg-red-500/20 text-red-400",
  expired: "bg-gray-500/20 text-gray-400",
  invalid: "bg-red-500/20 text-red-400",
};

function StatusBadge({ status }: { status: string }) {
  const colors = STATUS_COLORS[status] ?? "bg-gray-500/20 text-gray-400";
  return (
    <span
      className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${colors}`}
    >
      {status}
    </span>
  );
}

function ProposalList() {
  const proposalsResult = useAtomValue(proposalListAtom);
  const epochResult = useAtomValue(epochDurationAtom);
  const epochInfo = Result.builder(epochResult)
    .onSuccess((v) => v)
    .onInitial(() => null)
    .onFailure(() => null)
    .render();

  return (
    <section className="border border-border rounded-lg p-6 bg-card">
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider mb-3">
        Proposals
      </h2>
      {Result.builder(proposalsResult)
        .onInitial(() => (
          <div className="space-y-3">
            <div className="h-12 w-full bg-muted rounded animate-pulse" />
            <div className="h-12 w-full bg-muted rounded animate-pulse" />
          </div>
        ))
        .onSuccess((proposals) => {
          if (proposals.length === 0) {
            return (
              <p className="text-muted-foreground text-sm">
                No proposals yet.{" "}
                <Link
                  to="/proposals/new"
                  className="text-accent hover:underline"
                >
                  Create one
                </Link>
              </p>
            );
          }
          return (
            <div className="divide-y divide-border">
              {proposals.map((proposal) => (
                <ProposalRow
                  key={proposal.id}
                  proposal={proposal}
                  epochInfo={epochInfo}
                />
              ))}
            </div>
          );
        })
        .onFailure((error) => (
          <div className="text-red-400 text-sm">
            <p>Failed to load proposals.</p>
            <p className="text-xs mt-1 text-muted-foreground">
              {String(error)}
            </p>
          </div>
        ))
        .render()}
    </section>
  );
}

function ProposalRow({
  proposal,
  epochInfo,
}: {
  proposal: Proposal;
  epochInfo: { currentEpoch: number; secondsPerEpoch: number } | null;
}) {
  const created = new Date(proposal.created_at).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });

  const isTerminal =
    proposal.status === "committed" ||
    proposal.status === "failed" ||
    proposal.status === "expired" ||
    proposal.status === "invalid";

  const timeEstimate =
    epochInfo && !isTerminal
      ? formatEpochDelta(
          proposal.epoch_max - epochInfo.currentEpoch,
          epochInfo.secondsPerEpoch
        )
      : null;

  return (
    <Link
      to="/proposals/$id"
      params={{ id: proposal.id }}
      className="flex items-center justify-between py-3 hover:bg-muted/30 -mx-2 px-2 rounded transition-colors"
    >
      <div className="flex items-center gap-3 min-w-0">
        <StatusBadge status={proposal.status} />
        <div className="min-w-0">
          <code className="text-sm font-mono truncate block">
            {proposal.id.slice(0, 8)}...
          </code>
          <p className="text-xs text-muted-foreground">
            {timeEstimate ? (
              <>
                {timeEstimate}
                <span className="text-muted-foreground/60">
                  {" "}
                  · epochs {proposal.epoch_min}–{proposal.epoch_max}
                </span>
              </>
            ) : (
              <>
                Epochs {proposal.epoch_min}–{proposal.epoch_max}
              </>
            )}
          </p>
        </div>
      </div>
      <span className="text-xs text-muted-foreground whitespace-nowrap ml-4">
        {created}
      </span>
    </Link>
  );
}

function WalletStatusSkeleton() {
  return (
    <section className="border border-border rounded-lg p-6 bg-card">
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider mb-3">
        Connected Wallet
      </h2>
      <p className="text-muted-foreground">Connecting to wallet...</p>
    </section>
  );
}

function AccessRuleSkeleton() {
  return (
    <section className="border border-border rounded-lg p-6 bg-card">
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider mb-1">
        Multisig Access Rule
      </h2>
      <div className="mt-4 space-y-3">
        <div className="h-6 w-48 bg-muted rounded animate-pulse" />
        <div className="h-4 w-full bg-muted rounded animate-pulse" />
        <div className="h-4 w-full bg-muted rounded animate-pulse" />
      </div>
    </section>
  );
}

function WalletStatus() {
  const walletResult = useAtomValue(walletDataAtom);

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
          if (!walletData) return null;
          const accounts = walletData.accounts ?? [];
          if (accounts.length === 0) {
            return (
              <p className="text-muted-foreground">
                No accounts connected. Click the connect button above.
              </p>
            );
          }
          return (
            <div className="space-y-2">
              {accounts.map((account) => (
                <div
                  key={account.address}
                  className="flex items-center gap-3 text-sm"
                >
                  <span className="font-medium">{account.label}</span>
                  <a
                    href={`${dashboardBaseUrl(envVars.NETWORK_ID)}/account/${account.address}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-xs text-muted-foreground font-mono hover:text-accent transition-colors"
                  >
                    {account.address.slice(0, 20)}...
                    {account.address.slice(-8)}
                  </a>
                </div>
              ))}
            </div>
          );
        })
        .onFailure(() => (
          <p className="text-muted-foreground">
            Wallet not available. Install the Radix Wallet extension.
          </p>
        ))
        .render()}
    </section>
  );
}

function AccessRuleDisplay() {
  const accessRuleResult = useAtomValue(accessRuleAtom);

  return (
    <section className="border border-border rounded-lg p-6 bg-card">
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider mb-1">
        Multisig Access Rule
      </h2>
      <p className="text-xs text-muted-foreground mb-4 font-mono">
        {envVars.MULTISIG_ACCOUNT_ADDRESS ? (
          <>
            Account:{" "}
            <a
              href={`${dashboardBaseUrl(envVars.NETWORK_ID)}/account/${envVars.MULTISIG_ACCOUNT_ADDRESS}`}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-accent transition-colors"
            >
              {envVars.MULTISIG_ACCOUNT_ADDRESS.slice(0, 20)}...
            </a>
          </>
        ) : (
          "Account not configured"
        )}
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
  );
}

function SignerRow({ signer, index }: { signer: SignerInfo; index: number }) {
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
  );
}
