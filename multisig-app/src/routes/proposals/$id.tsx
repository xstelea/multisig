import { createFileRoute, Link } from "@tanstack/react-router";
import { Result, useAtomValue } from "@effect-atom/atom-react";
import { makeProposalDetailAtom } from "@/atom/proposalAtoms";
import { ClientOnly } from "@/lib/ClientOnly";
import { useMemo } from "react";
import type { Proposal } from "@/atom/orchestratorClient";

export const Route = createFileRoute("/proposals/$id")({
  component: ProposalDetailPage,
});

function ProposalDetailPage() {
  const { id } = Route.useParams();

  return (
    <div className="space-y-6">
      <div>
        <Link
          to="/"
          className="text-sm text-muted-foreground hover:text-foreground transition-colors"
        >
          &larr; Back to dashboard
        </Link>
        <h1 className="text-2xl font-semibold tracking-tight mt-2">
          Proposal Detail
        </h1>
      </div>

      <ClientOnly fallback={<DetailSkeleton />}>
        {() => <ProposalDetail id={id} />}
      </ClientOnly>
    </div>
  );
}

function DetailSkeleton() {
  return (
    <div className="space-y-4">
      <div className="h-8 w-48 bg-muted rounded animate-pulse" />
      <div className="h-48 w-full bg-muted rounded animate-pulse" />
      <div className="h-24 w-full bg-muted rounded animate-pulse" />
    </div>
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

function ProposalDetail({ id }: { id: string }) {
  const atom = useMemo(() => makeProposalDetailAtom(id), [id]);
  const result = useAtomValue(atom);

  return Result.builder(result)
    .onInitial(() => <DetailSkeleton />)
    .onSuccess((proposal) => <ProposalContent proposal={proposal} />)
    .onFailure((error) => (
      <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">
        <p>Failed to load proposal.</p>
        <p className="text-xs mt-1 text-muted-foreground">{String(error)}</p>
      </div>
    ))
    .render();
}

function ProposalContent({ proposal }: { proposal: Proposal }) {
  const created = new Date(proposal.created_at).toLocaleString("en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  });

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center gap-3">
        <StatusBadge status={proposal.status} />
        <code className="text-sm font-mono text-muted-foreground">
          {proposal.id}
        </code>
      </div>

      {/* Metadata grid */}
      <div className="grid grid-cols-2 gap-4">
        <MetadataField label="Created" value={created} />
        <MetadataField
          label="Epoch Window"
          value={`${proposal.epoch_min} – ${proposal.epoch_max}`}
        />
        {proposal.subintent_hash && (
          <MetadataField
            label="Subintent Hash"
            value={proposal.subintent_hash}
            mono
            className="col-span-2"
          />
        )}
        {proposal.tx_id && (
          <MetadataField
            label="Transaction ID"
            value={proposal.tx_id}
            mono
            className="col-span-2"
          />
        )}
      </div>

      {/* Manifest text */}
      <section className="space-y-2">
        <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">
          Transaction Manifest
        </h2>
        <pre className="bg-muted border border-border rounded-lg p-4 text-sm font-mono overflow-x-auto whitespace-pre-wrap break-words">
          {proposal.manifest_text}
        </pre>
      </section>
    </div>
  );
}

function MetadataField({
  label,
  value,
  mono = false,
  className = "",
}: {
  label: string;
  value: string;
  mono?: boolean;
  className?: string;
}) {
  return (
    <div className={`border border-border rounded-lg p-3 bg-card ${className}`}>
      <p className="text-xs text-muted-foreground mb-1">{label}</p>
      <p className={`text-sm ${mono ? "font-mono break-all" : ""}`}>{value}</p>
    </div>
  );
}
