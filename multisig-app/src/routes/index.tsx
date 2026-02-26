import { createFileRoute, Link } from "@tanstack/react-router";
import { Result, useAtomValue, useAtomMount } from "@effect-atom/atom-react";
import { dappToolkitAtom } from "@/atom/accessRuleAtom";
import { proposalListAtom } from "@/atom/proposalAtoms";
import { epochDurationAtom } from "@/atom/gatewayAtoms";
import { formatEpochDelta } from "@/lib/epochTime";
import { ClientOnly } from "@/lib/ClientOnly";
import type { Proposal } from "@/atom/orchestratorClient";

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
            Manage proposals for multisig accounts.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Link
            to="/create-account"
            className="inline-flex items-center gap-2 rounded-md border border-border px-4 py-2 text-sm font-medium text-foreground hover:bg-muted transition-colors"
          >
            Create Account
          </Link>
          <Link
            to="/proposals/new"
            className="inline-flex items-center gap-2 rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/80 transition-colors"
          >
            New Proposal
          </Link>
        </div>
      </div>

      <ClientOnly fallback={<ProposalListSkeleton />}>
        {() => <HomePageClient />}
      </ClientOnly>
    </div>
  );
}

function HomePageClient() {
  useAtomMount(dappToolkitAtom);
  return <ProposalList />;
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

  const shortAccount =
    proposal.multisig_account.length > 24
      ? `${proposal.multisig_account.slice(0, 16)}...${proposal.multisig_account.slice(-6)}`
      : proposal.multisig_account;

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
            <span className="text-muted-foreground/60"> · {shortAccount}</span>
          </p>
        </div>
      </div>
      <span className="text-xs text-muted-foreground whitespace-nowrap ml-4">
        {created}
      </span>
    </Link>
  );
}
