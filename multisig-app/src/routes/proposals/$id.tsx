import { createFileRoute, Link } from "@tanstack/react-router";
import {
  Result,
  useAtomValue,
  useAtomSet,
  useAtomMount,
} from "@effect-atom/atom-react";
import {
  makeProposalDetailAtom,
  makeSignatureStatusAtom,
  makeSignProposalAtom,
  makePrepareSubmissionAtom,
  makeSubmitProposalAtom,
} from "@/atom/proposalAtoms";
import { walletDataAtom, dappToolkitAtom } from "@/atom/accessRuleAtom";
import { ClientOnly } from "@/lib/ClientOnly";
import { useMemo, useState, useCallback } from "react";
import type {
  Proposal,
  SignatureStatusType,
  SignerStatus,
  SubmitProposalResponse,
} from "@/atom/orchestratorClient";
import { SubintentRequestBuilder } from "@radixdlt/radix-dapp-toolkit";

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

  useAtomMount(dappToolkitAtom);

  const signAtom = useMemo(
    () => makeSignProposalAtom(proposalAtom, sigStatusAtom),
    [proposalAtom, sigStatusAtom]
  );
  const prepareAtom = useMemo(() => makePrepareSubmissionAtom(id), [id]);
  const submitAtom = useMemo(
    () => makeSubmitProposalAtom(proposalAtom),
    [proposalAtom]
  );

  return Result.builder(proposalResult)
    .onInitial(() => <DetailSkeleton />)
    .onSuccess((proposal) => (
      <ProposalContent
        proposal={proposal}
        sigStatusAtom={sigStatusAtom}
        signAtom={signAtom}
        prepareAtom={prepareAtom}
        submitAtom={submitAtom}
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
  signAtom,
  prepareAtom,
  submitAtom,
}: {
  proposal: Proposal;
  sigStatusAtom: ReturnType<typeof makeSignatureStatusAtom>;
  signAtom: ReturnType<typeof makeSignProposalAtom>;
  prepareAtom: ReturnType<typeof makePrepareSubmissionAtom>;
  submitAtom: ReturnType<typeof makeSubmitProposalAtom>;
}) {
  const created = new Date(proposal.created_at).toLocaleString("en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  });

  const canSign =
    proposal.status === "created" || proposal.status === "signing";

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

      {/* Signature progress */}
      <SignatureProgress
        sigStatusAtom={sigStatusAtom}
        canSign={canSign}
        proposal={proposal}
        signAtom={signAtom}
      />

      {/* Submit section — shown when proposal is ready */}
      {proposal.status === "ready" && (
        <SubmitSection
          proposal={proposal}
          prepareAtom={prepareAtom}
          submitAtom={submitAtom}
        />
      )}

      {/* Transaction result — shown when committed or failed */}
      {(proposal.status === "committed" || proposal.status === "failed") && (
        <TransactionResult proposal={proposal} />
      )}

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
  signAtom,
}: {
  sigStatusAtom: ReturnType<typeof makeSignatureStatusAtom>;
  canSign: boolean;
  proposal: Proposal;
  signAtom: ReturnType<typeof makeSignProposalAtom>;
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
            signAtom={signAtom}
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
  signAtom,
}: {
  sigStatus: SignatureStatusType;
  canSign: boolean;
  proposal: Proposal;
  signAtom: ReturnType<typeof makeSignProposalAtom>;
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
        <SignButton
          proposalId={proposal.id}
          manifest={proposal.manifest_text}
          signAtom={signAtom}
        />
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
  proposalId,
  manifest,
  signAtom,
}: {
  proposalId: string;
  manifest: string;
  signAtom: ReturnType<typeof makeSignProposalAtom>;
}) {
  const walletResult = useAtomValue(walletDataAtom);
  const rdtResult = useAtomValue(dappToolkitAtom);
  const signProposal = useAtomSet(signAtom, { mode: "promise" });
  const [signing, setSigning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isConnected = Result.builder(walletResult)
    .onSuccess((data) => (data?.accounts?.length ?? 0) > 0)
    .onInitial(() => false)
    .onFailure(() => false)
    .render();

  const handleSign = useCallback(async () => {
    setError(null);
    setSigning(true);
    try {
      const rdt = Result.builder(rdtResult)
        .onSuccess((r) => r)
        .onInitial(() => null)
        .onFailure(() => null)
        .render();

      if (!rdt) {
        setError("Wallet not connected");
        setSigning(false);
        return;
      }

      // Build manifest with YIELD_TO_PARENT if not present
      const subintentManifest = manifest.includes("YIELD_TO_PARENT")
        ? manifest
        : `${manifest.trimEnd()}\nYIELD_TO_PARENT;\n`;

      // Send pre-authorization request to wallet via SubintentRequestBuilder
      const result = await rdt.walletApi.sendPreAuthorizationRequest(
        SubintentRequestBuilder()
          .manifest(subintentManifest)
          .setExpiration("afterDelay", 3600)
      );

      if (result.isErr()) {
        setError(`Wallet error: ${result.error.message ?? "Unknown error"}`);
        setSigning(false);
        return;
      }

      const { signedPartialTransaction } = result.value;

      // Send signed partial to backend
      await signProposal({
        proposalId,
        signedPartialTransactionHex: signedPartialTransaction,
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setSigning(false);
    }
  }, [rdtResult, manifest, signProposal, proposalId]);

  if (!isConnected) {
    return (
      <p className="text-sm text-muted-foreground">
        Connect your wallet to sign this proposal.
      </p>
    );
  }

  return (
    <div className="space-y-2">
      <button
        onClick={handleSign}
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
  prepareAtom,
  submitAtom,
}: {
  proposal: Proposal;
  prepareAtom: ReturnType<typeof makePrepareSubmissionAtom>;
  submitAtom: ReturnType<typeof makeSubmitProposalAtom>;
}) {
  const walletResult = useAtomValue(walletDataAtom);
  const rdtResult = useAtomValue(dappToolkitAtom);
  const prepareSubmission = useAtomSet(prepareAtom, { mode: "promise" });
  const submitProposal = useAtomSet(submitAtom, { mode: "promise" });

  const [submitting, setSubmitting] = useState(false);
  const [step, setStep] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SubmitProposalResponse | null>(null);

  const connectedAccount = Result.builder(walletResult)
    .onSuccess((data) => data?.accounts?.[0]?.address ?? null)
    .onInitial(() => null)
    .onFailure(() => null)
    .render();

  const handleSubmit = useCallback(async () => {
    if (!connectedAccount) return;

    setError(null);
    setResult(null);
    setSubmitting(true);

    try {
      // Step 1: Get fee manifest from backend
      setStep("Preparing fee payment...");
      const prepared = await prepareSubmission(connectedAccount);

      // Step 2: Send fee manifest to wallet for signing
      setStep("Sign fee payment in your wallet...");
      const rdt = Result.builder(rdtResult)
        .onSuccess((r) => r)
        .onInitial(() => null)
        .onFailure(() => null)
        .render();

      if (!rdt) {
        setError("Wallet not connected");
        setSubmitting(false);
        setStep(null);
        return;
      }

      const walletResult2 = await rdt.walletApi.sendPreAuthorizationRequest(
        SubintentRequestBuilder()
          .manifest(prepared.fee_manifest)
          .setExpiration("afterDelay", 3600)
      );

      if (walletResult2.isErr()) {
        setError(
          `Wallet error: ${walletResult2.error.message ?? "Unknown error"}`
        );
        setSubmitting(false);
        setStep(null);
        return;
      }

      // Step 3: Submit to backend for composition + Gateway submission
      setStep("Submitting transaction...");
      const submitResult = await submitProposal({
        proposalId: proposal.id,
        signedFeePaymentHex: walletResult2.value.signedPartialTransaction,
        feePayerAccount: connectedAccount,
      });

      setResult(submitResult);
    } catch (e) {
      setError(String(e));
    } finally {
      setSubmitting(false);
      setStep(null);
    }
  }, [
    connectedAccount,
    rdtResult,
    prepareSubmission,
    submitProposal,
    proposal.id,
  ]);

  if (!connectedAccount) {
    return (
      <section className="border border-border rounded-lg p-6 bg-card space-y-3">
        <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">
          Submit Transaction
        </h2>
        <p className="text-sm text-muted-foreground">
          Connect your wallet to submit this proposal as the fee payer.
        </p>
      </section>
    );
  }

  return (
    <section className="border border-border rounded-lg p-6 bg-card space-y-4">
      <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">
        Submit Transaction
      </h2>
      <p className="text-sm text-muted-foreground">
        All signatures collected. Submit this proposal by paying the transaction
        fee.
      </p>

      <div className="space-y-3">
        <button
          onClick={handleSubmit}
          disabled={submitting}
          className="inline-flex items-center gap-2 rounded-md bg-green-600 px-4 py-2 text-sm font-medium text-white hover:bg-green-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {submitting ? "Submitting..." : "Pay Fee & Submit"}
        </button>

        {step && (
          <p className="text-sm text-muted-foreground animate-pulse">{step}</p>
        )}

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
