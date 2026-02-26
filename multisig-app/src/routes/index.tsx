import { createFileRoute, Link } from "@tanstack/react-router";
import { Result, useAtomValue, useAtomMount } from "@effect-atom/atom-react";
import { dappToolkitAtom } from "@/atom/accessRuleAtom";
import { proposalListAtom } from "@/atom/proposalAtoms";
import { epochDurationAtom } from "@/atom/gatewayAtoms";
import { formatEpochDelta } from "@/lib/epochTime";
import { ClientOnly } from "@/lib/ClientOnly";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { SectionCard } from "@/components/section-card";
import { StatusBadge } from "@/components/status-badge";
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
          <Button asChild variant="outline">
            <Link to="/create-account">Create Account</Link>
          </Button>
          <Button asChild variant="accent">
            <Link to="/proposals/new">New Proposal</Link>
          </Button>
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
    <SectionCard title="Proposals">
      <div className="space-y-3">
        <Skeleton className="h-12 w-full" />
        <Skeleton className="h-12 w-full" />
      </div>
    </SectionCard>
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
    <SectionCard title="Proposals">
      {Result.builder(proposalsResult)
        .onInitial(() => (
          <div className="space-y-3">
            <Skeleton className="h-12 w-full" />
            <Skeleton className="h-12 w-full" />
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
          <Alert variant="destructive">
            <AlertTitle>Failed to load proposals.</AlertTitle>
            <AlertDescription>{String(error)}</AlertDescription>
          </Alert>
        ))
        .render()}
    </SectionCard>
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
