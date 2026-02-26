import { createFileRoute } from "@tanstack/react-router";
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
import { envVars, dashboardBaseUrl } from "@/lib/envVars";
import { ClientOnly } from "@/lib/ClientOnly";
import { useMemo, useState, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import { Progress } from "@/components/ui/progress";
import { Card } from "@/components/ui/card";
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui/table";
import { SectionCard } from "@/components/section-card";
import { StatusBadge } from "@/components/status-badge";
import { BackLink } from "@/components/back-link";
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
        <BackLink />
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
      <Skeleton className="h-8 w-48" />
      <Skeleton className="h-48 w-full" />
      <Skeleton className="h-24 w-full" />
    </div>
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
      <Alert variant="destructive">
        <AlertTitle>Failed to load proposal.</AlertTitle>
        <AlertDescription>{String(error)}</AlertDescription>
      </Alert>
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
        <Card className="p-3 col-span-2">
          <p className="text-xs text-muted-foreground mb-1">Multisig Account</p>
          <a
            href={`${dashboardBaseUrl(envVars.NETWORK_ID)}/account/${proposal.multisig_account}`}
            target="_blank"
            rel="noopener noreferrer"
            className="text-sm font-mono break-all hover:text-accent transition-colors"
          >
            {proposal.multisig_account}
          </a>
        </Card>
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
      <SignatureProgressSection
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

function SignatureProgressSection({
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
    <SectionCard title="Signatures">
      {Result.builder(sigStatusResult)
        .onInitial(() => (
          <div className="space-y-2">
            <Skeleton className="h-6 w-48" />
            <Skeleton className="h-4 w-full" />
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
    </SectionCard>
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
  const pct = Math.min(100, (sigStatus.collected / sigStatus.threshold) * 100);

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
        <Progress value={pct} />
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
      <Button onClick={onSign} disabled={signing} variant="accent">
        {signing ? "Signing..." : "Sign Proposal"}
      </Button>
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
    <SectionCard title="Submit Transaction">
      <p className="text-sm text-muted-foreground">
        All signatures collected. The server will pay the transaction fee and
        submit to the network.
      </p>

      <div className="mt-4 space-y-3">
        <Button
          onClick={handleSubmit}
          disabled={submitting}
          className="bg-green-600 text-white hover:bg-green-500"
        >
          {submitting ? "Submitting..." : "Submit"}
        </Button>

        {error && <p className="text-sm text-red-400">{error}</p>}

        {result && (
          <Alert
            className={
              result.status === "committed"
                ? "border-emerald-500/20 bg-emerald-500/10 text-emerald-400"
                : "border-red-500/20 bg-red-500/10 text-red-400"
            }
          >
            <AlertTitle>
              {result.status === "committed"
                ? "Transaction committed!"
                : `Transaction ${result.status}`}
            </AlertTitle>
            <AlertDescription>
              {result.tx_id && (
                <p className="font-mono text-xs break-all">
                  TX: {result.tx_id}
                </p>
              )}
              {result.error && <p className="text-xs">{result.error}</p>}
            </AlertDescription>
          </Alert>
        )}
      </div>
    </SectionCard>
  );
}

function TransactionResult({ proposal }: { proposal: Proposal }) {
  const isCommitted = proposal.status === "committed";

  return (
    <Alert
      className={
        isCommitted
          ? "border-emerald-500/30 bg-emerald-500/5"
          : "border-red-500/30 bg-red-500/5"
      }
    >
      <AlertTitle className="text-sm font-medium text-muted-foreground uppercase tracking-wider">
        Transaction Result
      </AlertTitle>
      <AlertDescription>
        <p
          className={`text-sm font-medium ${isCommitted ? "text-emerald-400" : "text-red-400"}`}
        >
          {isCommitted ? "Committed on ledger" : "Transaction failed"}
        </p>
        {proposal.tx_id && (
          <p className="font-mono text-xs text-muted-foreground break-all mt-1">
            {proposal.tx_id}
          </p>
        )}
        {proposal.submitted_at && (
          <p className="text-xs text-muted-foreground mt-1">
            Submitted{" "}
            {new Date(proposal.submitted_at).toLocaleString("en-US", {
              dateStyle: "medium",
              timeStyle: "short",
            })}
          </p>
        )}
      </AlertDescription>
    </Alert>
  );
}

function ValidityWarning({ proposal }: { proposal: Proposal }) {
  const isExpired = proposal.status === "expired";

  return (
    <Alert
      className={
        isExpired
          ? "border-gray-500/30 bg-gray-500/5"
          : "border-red-500/30 bg-red-500/5"
      }
    >
      <AlertTitle className={isExpired ? "text-gray-400" : "text-red-400"}>
        {isExpired ? "Proposal Expired" : "Proposal Invalid"}
      </AlertTitle>
      {proposal.invalid_reason && (
        <AlertDescription>{proposal.invalid_reason}</AlertDescription>
      )}
    </Alert>
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
    <SectionCard title="Transaction Preview">
      <p className="text-sm text-muted-foreground">
        Simulate execution to estimate fees and check for errors before signing.
      </p>

      <div className="mt-4">
        <Button onClick={handlePreview} disabled={loading} variant="outline">
          {loading ? "Previewing..." : "Run Preview"}
        </Button>
      </div>

      {error && (
        <Alert variant="destructive" className="mt-4">
          <AlertTitle>Preview failed</AlertTitle>
          <AlertDescription className="break-all">{error}</AlertDescription>
        </Alert>
      )}

      {result && (
        <div className="mt-4 space-y-4">
          {/* Status */}
          <Alert
            className={
              isSuccess
                ? "border-emerald-500/20 bg-emerald-500/10 text-emerald-400"
                : "border-red-500/20 bg-red-500/10 text-red-400"
            }
          >
            <AlertTitle>
              {isSuccess ? "Simulation Succeeded" : `Simulation: ${status}`}
            </AlertTitle>
            {receipt?.error_message && (
              <AlertDescription>{receipt.error_message}</AlertDescription>
            )}
          </Alert>

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
    </SectionCard>
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
          <Card key={label} className="p-3">
            <p className="text-xs text-muted-foreground">{label}</p>
            <p className="text-sm font-mono">{value}</p>
          </Card>
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
        <Table>
          <TableHeader>
            <TableRow className="bg-muted/50 text-xs text-muted-foreground">
              <TableHead>Account</TableHead>
              <TableHead>Resource</TableHead>
              <TableHead className="text-right">Amount</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((row, i) => {
              const isNegative = row.amount.startsWith("-");
              return (
                <TableRow key={i}>
                  <TableCell className="font-mono text-xs" title={row.account}>
                    {shortAddr(row.account)}
                  </TableCell>
                  <TableCell className="font-mono text-xs" title={row.resource}>
                    {row.resource.includes("xrd")
                      ? "XRD"
                      : shortAddr(row.resource)}
                  </TableCell>
                  <TableCell
                    className={`font-mono text-xs text-right font-medium ${
                      isNegative ? "text-red-400" : "text-emerald-400"
                    }`}
                  >
                    {isNegative ? row.amount : `+${row.amount}`}
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
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
    <Card className={`p-3 ${className}`}>
      <p className="text-xs text-muted-foreground mb-1">{label}</p>
      <p className={`text-sm ${mono ? "font-mono break-all" : ""}`}>{value}</p>
    </Card>
  );
}
