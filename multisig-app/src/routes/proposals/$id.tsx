import { createFileRoute, Link } from "@tanstack/react-router";
import { Result, useAtomValue, useAtomSet } from "@effect-atom/atom-react";
import {
  makeProposalDetailAtom,
  makeSignatureStatusAtom,
  makeSubmitProposalAtom,
} from "@/atom/proposalAtoms";
import { makeHandleSignAtom } from "@/atom/handleSignAtom";
import { makePreviewTransactionAtom } from "@/atom/previewAtom";
import { epochDurationAtom } from "@/atom/gatewayAtoms";
import { formatEpochDelta } from "@/lib/epochTime";
import { ClientOnly } from "@/lib/ClientOnly";
import { useMemo, useState, useCallback } from "react";
import type {
  Proposal,
  SignatureStatusType,
  SignerStatus,
  SubmitProposalResponse,
} from "@/atom/orchestratorClient";

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
  const proposalAtom = useMemo(() => makeProposalDetailAtom(id), [id]);
  const sigStatusAtom = useMemo(() => makeSignatureStatusAtom(id), [id]);
  const proposalResult = useAtomValue(proposalAtom);

  const handleSignAtom = useMemo(
    () => makeHandleSignAtom(proposalAtom, sigStatusAtom),
    [proposalAtom, sigStatusAtom]
  );
  const submitAtom = useMemo(
    () => makeSubmitProposalAtom(proposalAtom),
    [proposalAtom]
  );
  const previewAtom = useMemo(() => makePreviewTransactionAtom(), []);

  return Result.builder(proposalResult)
    .onInitial(() => <DetailSkeleton />)
    .onSuccess((proposal) => (
      <ProposalContent
        proposal={proposal}
        sigStatusAtom={sigStatusAtom}
        handleSignAtom={handleSignAtom}
        submitAtom={submitAtom}
        previewAtom={previewAtom}
      />
    ))
    .onFailure((error) => (
      <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">
        <p>Failed to load proposal.</p>
        <p className="text-xs mt-1 text-muted-foreground">{String(error)}</p>
      </div>
    ))
    .render();
}

function ProposalContent({
  proposal,
  sigStatusAtom,
  handleSignAtom,
  submitAtom,
  previewAtom,
}: {
  proposal: Proposal;
  sigStatusAtom: ReturnType<typeof makeSignatureStatusAtom>;
  handleSignAtom: ReturnType<typeof makeHandleSignAtom>;
  submitAtom: ReturnType<typeof makeSubmitProposalAtom>;
  previewAtom: ReturnType<typeof makePreviewTransactionAtom>;
}) {
  const created = new Date(proposal.created_at).toLocaleString("en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  });

  const canSign =
    proposal.status === "created" || proposal.status === "signing";

  const epochResult = useAtomValue(epochDurationAtom);
  const epochInfo = Result.builder(epochResult)
    .onSuccess((v) => v)
    .onInitial(() => null)
    .onFailure(() => null)
    .render();

  const isTerminal =
    proposal.status === "committed" ||
    proposal.status === "failed" ||
    proposal.status === "expired" ||
    proposal.status === "invalid";

  const epochWindowLabel = (() => {
    const base = `${proposal.epoch_min} – ${proposal.epoch_max}`;
    if (!epochInfo || isTerminal) return base;
    const remaining = formatEpochDelta(
      proposal.epoch_max - epochInfo.currentEpoch,
      epochInfo.secondsPerEpoch
    );
    return `${base} (${remaining} remaining)`;
  })();

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center gap-3">
        <StatusBadge status={proposal.status} />
        <code className="text-sm font-mono text-muted-foreground">
          {proposal.id}
        </code>
      </div>

      {/* Validity warning banner */}
      {(proposal.status === "expired" || proposal.status === "invalid") && (
        <ValidityWarning proposal={proposal} />
      )}

      {/* Metadata grid */}
      <div className="grid grid-cols-2 gap-4">
        <MetadataField label="Created" value={created} />
        <MetadataField label="Epoch Window" value={epochWindowLabel} />
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

      {/* Signature progress */}
      <SignatureProgress
        sigStatusAtom={sigStatusAtom}
        canSign={canSign}
        proposal={proposal}
        handleSignAtom={handleSignAtom}
      />

      {/* Submit section — shown when proposal is ready */}
      {proposal.status === "ready" && (
        <SubmitSection proposal={proposal} submitAtom={submitAtom} />
      )}

      {/* Transaction result — shown when committed or failed */}
      {(proposal.status === "committed" || proposal.status === "failed") && (
        <TransactionResult proposal={proposal} />
      )}

      {/* Transaction preview */}
      <TransactionPreview
        manifestText={proposal.manifest_text}
        previewAtom={previewAtom}
      />

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

function SignatureProgress({
  sigStatusAtom,
  canSign,
  proposal,
  handleSignAtom,
}: {
  sigStatusAtom: ReturnType<typeof makeSignatureStatusAtom>;
  canSign: boolean;
  proposal: Proposal;
  handleSignAtom: ReturnType<typeof makeHandleSignAtom>;
}) {
  const sigStatusResult = useAtomValue(sigStatusAtom);

  return (
    <section className="border border-border rounded-lg p-6 bg-card space-y-4">
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">
        Signatures
      </h2>

      {Result.builder(sigStatusResult)
        .onInitial(() => (
          <div className="space-y-2">
            <div className="h-6 w-48 bg-muted rounded animate-pulse" />
            <div className="h-4 w-full bg-muted rounded animate-pulse" />
          </div>
        ))
        .onSuccess((sigStatus) => (
          <SignatureStatusDisplay
            sigStatus={sigStatus}
            canSign={canSign}
            proposal={proposal}
            handleSignAtom={handleSignAtom}
          />
        ))
        .onFailure((error) => (
          <p className="text-sm text-red-400">
            Failed to load signatures: {String(error)}
          </p>
        ))
        .render()}
    </section>
  );
}

function SignatureStatusDisplay({
  sigStatus,
  canSign,
  proposal,
  handleSignAtom,
}: {
  sigStatus: SignatureStatusType;
  canSign: boolean;
  proposal: Proposal;
  handleSignAtom: ReturnType<typeof makeHandleSignAtom>;
}) {
  return (
    <div className="space-y-4">
      {/* Progress bar */}
      <div>
        <div className="flex items-baseline gap-2 mb-2">
          <span className="text-2xl font-bold">{sigStatus.collected}</span>
          <span className="text-muted-foreground">
            of {sigStatus.threshold} signatures collected
          </span>
          {sigStatus.remaining > 0 && (
            <span className="text-xs text-muted-foreground">
              ({sigStatus.remaining} more needed)
            </span>
          )}
        </div>
        <div className="w-full h-2 bg-muted rounded-full overflow-hidden">
          <div
            className="h-full bg-accent rounded-full transition-all"
            style={{
              width: `${Math.min(100, (sigStatus.collected / sigStatus.threshold) * 100)}%`,
            }}
          />
        </div>
      </div>

      {/* Signer list */}
      <div className="divide-y divide-border">
        {sigStatus.signers.map((signer) => (
          <SignerStatusRow key={signer.key_hash} signer={signer} />
        ))}
      </div>

      {/* Sign button */}
      {canSign && (
        <SignButton proposal={proposal} handleSignAtom={handleSignAtom} />
      )}
    </div>
  );
}

function SignerStatusRow({ signer }: { signer: SignerStatus }) {
  const invalidated = signer.has_signed && !signer.is_valid;

  return (
    <div
      className={`py-2.5 flex items-center justify-between ${invalidated ? "opacity-60" : ""}`}
    >
      <div className="flex items-center gap-3">
        <div
          className={`w-2 h-2 rounded-full ${
            invalidated
              ? "bg-red-400"
              : signer.has_signed
                ? "bg-green-400"
                : "bg-muted-foreground/30"
          }`}
        />
        <code
          className={`text-sm font-mono ${invalidated ? "line-through" : ""}`}
        >
          {signer.key_hash.slice(0, 12)}...{signer.key_hash.slice(-8)}
        </code>
        <span className="text-xs text-muted-foreground">{signer.key_type}</span>
      </div>
      <span
        className={`text-xs font-medium ${
          invalidated
            ? "text-red-400"
            : signer.has_signed
              ? "text-green-400"
              : "text-muted-foreground"
        }`}
      >
        {invalidated ? "Invalidated" : signer.has_signed ? "Signed" : "Pending"}
      </span>
    </div>
  );
}

function SignButton({
  proposal,
  handleSignAtom,
}: {
  proposal: Proposal;
  handleSignAtom: ReturnType<typeof makeHandleSignAtom>;
}) {
  const handleSign = useAtomSet(handleSignAtom, { mode: "promise" });
  const [signing, setSigning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onSign = useCallback(async () => {
    setError(null);
    setSigning(true);
    try {
      await handleSign(proposal);
    } catch (e) {
      setError(String(e));
    } finally {
      setSigning(false);
    }
  }, [handleSign, proposal]);

  return (
    <div className="space-y-2">
      <button
        onClick={onSign}
        disabled={signing}
        className="inline-flex items-center gap-2 rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/80 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {signing ? "Signing..." : "Sign Proposal"}
      </button>
      {error && <p className="text-sm text-red-400">{error}</p>}
    </div>
  );
}

function SubmitSection({
  proposal,
  submitAtom,
}: {
  proposal: Proposal;
  submitAtom: ReturnType<typeof makeSubmitProposalAtom>;
}) {
  const submitProposal = useAtomSet(submitAtom, { mode: "promise" });

  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SubmitProposalResponse | null>(null);

  const handleSubmit = useCallback(async () => {
    setError(null);
    setResult(null);
    setSubmitting(true);
    try {
      const submitResult = await submitProposal(proposal.id);
      setResult(submitResult);
    } catch (e) {
      setError(String(e));
    } finally {
      setSubmitting(false);
    }
  }, [submitProposal, proposal.id]);

  return (
    <section className="border border-border rounded-lg p-6 bg-card space-y-4">
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">
        Submit Transaction
      </h2>
      <p className="text-sm text-muted-foreground">
        All signatures collected. The server will pay the transaction fee and
        submit to the network.
      </p>

      <div className="space-y-3">
        <button
          onClick={handleSubmit}
          disabled={submitting}
          className="inline-flex items-center gap-2 rounded-md bg-green-600 px-4 py-2 text-sm font-medium text-white hover:bg-green-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {submitting ? "Submitting..." : "Submit"}
        </button>

        {error && <p className="text-sm text-red-400">{error}</p>}

        {result && (
          <div
            className={`rounded-lg px-4 py-3 text-sm ${
              result.status === "committed"
                ? "bg-emerald-500/10 border border-emerald-500/20 text-emerald-400"
                : "bg-red-500/10 border border-red-500/20 text-red-400"
            }`}
          >
            <p className="font-medium">
              {result.status === "committed"
                ? "Transaction committed!"
                : `Transaction ${result.status}`}
            </p>
            {result.tx_id && (
              <p className="mt-1 font-mono text-xs break-all">
                TX: {result.tx_id}
              </p>
            )}
            {result.error && <p className="mt-1 text-xs">{result.error}</p>}
          </div>
        )}
      </div>
    </section>
  );
}

function TransactionResult({ proposal }: { proposal: Proposal }) {
  const isCommitted = proposal.status === "committed";

  return (
    <section
      className={`border rounded-lg p-6 space-y-2 ${
        isCommitted
          ? "border-emerald-500/30 bg-emerald-500/5"
          : "border-red-500/30 bg-red-500/5"
      }`}
    >
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">
        Transaction Result
      </h2>
      <p
        className={`text-sm font-medium ${isCommitted ? "text-emerald-400" : "text-red-400"}`}
      >
        {isCommitted ? "Committed on ledger" : "Transaction failed"}
      </p>
      {proposal.tx_id && (
        <p className="font-mono text-xs text-muted-foreground break-all">
          {proposal.tx_id}
        </p>
      )}
      {proposal.submitted_at && (
        <p className="text-xs text-muted-foreground">
          Submitted{" "}
          {new Date(proposal.submitted_at).toLocaleString("en-US", {
            dateStyle: "medium",
            timeStyle: "short",
          })}
        </p>
      )}
    </section>
  );
}

function ValidityWarning({ proposal }: { proposal: Proposal }) {
  const isExpired = proposal.status === "expired";

  return (
    <section
      className={`border rounded-lg p-4 ${
        isExpired
          ? "border-gray-500/30 bg-gray-500/5"
          : "border-red-500/30 bg-red-500/5"
      }`}
    >
      <p
        className={`text-sm font-medium ${isExpired ? "text-gray-400" : "text-red-400"}`}
      >
        {isExpired ? "Proposal Expired" : "Proposal Invalid"}
      </p>
      {proposal.invalid_reason && (
        <p className="text-xs text-muted-foreground mt-1">
          {proposal.invalid_reason}
        </p>
      )}
    </section>
  );
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type PreviewResult = { receipt: any; resource_changes: any[]; logs: any[] };

function TransactionPreview({
  manifestText,
  previewAtom,
}: {
  manifestText: string;
  previewAtom: ReturnType<typeof makePreviewTransactionAtom>;
}) {
  const runPreview = useAtomSet(previewAtom, { mode: "promise" });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<PreviewResult | null>(null);

  const handlePreview = useCallback(async () => {
    setError(null);
    setResult(null);
    setLoading(true);
    try {
      const res = await runPreview(manifestText);
      setResult(res as PreviewResult);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [runPreview, manifestText]);

  const receipt = result?.receipt;
  const status: string | undefined = receipt?.status;
  const isSuccess = status === "CommitSuccess" || status === "Succeeded";

  return (
    <section className="border border-border rounded-lg p-6 bg-card space-y-4">
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">
        Transaction Preview
      </h2>
      <p className="text-sm text-muted-foreground">
        Simulate execution to estimate fees and check for errors before signing.
      </p>

      <button
        onClick={handlePreview}
        disabled={loading}
        className="inline-flex items-center gap-2 rounded-md bg-muted px-4 py-2 text-sm font-medium text-foreground hover:bg-muted/70 border border-border transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {loading ? "Previewing..." : "Run Preview"}
      </button>

      {error && (
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">
          <p className="font-medium">Preview failed</p>
          <p className="text-xs mt-1 break-all">{error}</p>
        </div>
      )}

      {result && (
        <div className="space-y-4">
          {/* Status */}
          <div
            className={`rounded-lg px-4 py-3 text-sm ${
              isSuccess
                ? "bg-emerald-500/10 border border-emerald-500/20 text-emerald-400"
                : "bg-red-500/10 border border-red-500/20 text-red-400"
            }`}
          >
            <p className="font-medium">
              {isSuccess ? "Simulation Succeeded" : `Simulation: ${status}`}
            </p>
            {receipt?.error_message && (
              <p className="text-xs mt-1">{receipt.error_message}</p>
            )}
          </div>

          {/* Fee summary */}
          {receipt?.fee_summary && (
            <FeeSummary feeSummary={receipt.fee_summary} />
          )}

          {/* Resource changes */}
          {result.resource_changes.length > 0 && (
            <ResourceChanges changes={result.resource_changes} />
          )}

          {/* Logs */}
          {result.logs.length > 0 && (
            <div className="space-y-2">
              <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                Logs
              </h3>
              <div className="bg-muted border border-border rounded-lg p-3 text-xs font-mono space-y-1 max-h-48 overflow-y-auto">
                {result.logs.map((log, i) => (
                  <div key={i} className="flex gap-2">
                    <span className="text-muted-foreground shrink-0">
                      [{log.level}]
                    </span>
                    <span className="break-all">{log.message}</span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </section>
  );
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function FeeSummary({ feeSummary }: { feeSummary: any }) {
  const fields = [
    ["Execution Cost (XRD)", feeSummary.xrd_total_execution_cost],
    ["Finalization Cost (XRD)", feeSummary.xrd_total_finalization_cost],
    ["Storage Cost (XRD)", feeSummary.xrd_total_storage_cost],
    ["Royalty Cost (XRD)", feeSummary.xrd_total_royalty_cost],
    ["Tipping Cost (XRD)", feeSummary.xrd_total_tipping_cost],
  ].filter(([, v]) => v != null);

  if (fields.length === 0) return null;

  return (
    <div className="space-y-2">
      <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
        Fee Estimate
      </h3>
      <div className="grid grid-cols-2 gap-2">
        {fields.map(([label, value]) => (
          <div
            key={label}
            className="border border-border rounded-lg p-2 bg-muted/50"
          >
            <p className="text-xs text-muted-foreground">{label}</p>
            <p className="text-sm font-mono">{value}</p>
          </div>
        ))}
      </div>
    </div>
  );
}

/** Truncate an address to first 12 and last 8 chars. */
const shortAddr = (addr: string) =>
  addr.length > 24 ? `${addr.slice(0, 12)}...${addr.slice(-8)}` : addr;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function ResourceChanges({ changes }: { changes: any[] }) {
  // Flatten nested resource_changes from each instruction index
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const rows: { account: string; resource: string; amount: string }[] = [];
  for (const entry of changes) {
    for (const rc of entry.resource_changes ?? []) {
      rows.push({
        account: rc.component_entity?.entity_address ?? "unknown",
        resource: rc.resource_address ?? "unknown",
        amount: rc.amount ?? "0",
      });
    }
  }

  if (rows.length === 0) return null;

  return (
    <div className="space-y-2">
      <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
        Resource Changes
      </h3>
      <div className="border border-border rounded-lg overflow-hidden">
        <table className="w-full text-sm">
          <thead>
            <tr className="bg-muted/50 text-xs text-muted-foreground">
              <th className="text-left px-3 py-2 font-medium">Account</th>
              <th className="text-left px-3 py-2 font-medium">Resource</th>
              <th className="text-right px-3 py-2 font-medium">Amount</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {rows.map((row, i) => {
              const isNegative = row.amount.startsWith("-");
              return (
                <tr key={i}>
                  <td
                    className="px-3 py-2 font-mono text-xs"
                    title={row.account}
                  >
                    {shortAddr(row.account)}
                  </td>
                  <td
                    className="px-3 py-2 font-mono text-xs"
                    title={row.resource}
                  >
                    {row.resource.includes("xrd")
                      ? "XRD"
                      : shortAddr(row.resource)}
                  </td>
                  <td
                    className={`px-3 py-2 font-mono text-xs text-right font-medium ${
                      isNegative ? "text-red-400" : "text-emerald-400"
                    }`}
                  >
                    {isNegative ? row.amount : `+${row.amount}`}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
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
