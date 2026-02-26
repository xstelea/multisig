import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { Result, useAtomSet, useAtomValue } from "@effect-atom/atom-react";
import { createProposalAtom } from "@/atom/proposalAtoms";
import { epochDurationAtom } from "@/atom/gatewayAtoms";
import { formatEpochDelta, hoursToEpochs } from "@/lib/epochTime";
import { ClientOnly } from "@/lib/ClientOnly";
import { useState } from "react";

export const Route = createFileRoute("/proposals/new")({
  component: NewProposalPage,
});

function NewProposalPage() {
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
          Create Proposal
        </h1>
        <p className="text-muted-foreground mt-1">
          Paste a raw transaction manifest and set an expiry epoch.
        </p>
      </div>

      <ClientOnly fallback={<FormSkeleton />}>
        {() => <CreateProposalForm />}
      </ClientOnly>
    </div>
  );
}

function FormSkeleton() {
  return (
    <div className="space-y-4">
      <div className="h-48 bg-muted rounded animate-pulse" />
      <div className="h-10 w-32 bg-muted rounded animate-pulse" />
    </div>
  );
}

function CreateProposalForm() {
  const navigate = useNavigate();
  const [multisigAccount, setMultisigAccount] = useState("");
  const [manifestText, setManifestText] = useState("");
  const [expiryMode, setExpiryMode] = useState<"hours" | "epoch">("hours");
  const [expiryHours, setExpiryHours] = useState("");
  const [expiryEpoch, setExpiryEpoch] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const epochDurationResult = useAtomValue(epochDurationAtom);
  const epochDuration = Result.builder(epochDurationResult)
    .onSuccess((v) => v)
    .orElse(() => undefined);

  const currentEpoch = epochDuration?.currentEpoch;
  const secondsPerEpoch = epochDuration?.secondsPerEpoch;

  const createProposal = useAtomSet(createProposalAtom, { mode: "promise" });

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!multisigAccount.trim()) {
      setError("Multisig account address is required");
      return;
    }

    if (!manifestText.trim()) {
      setError("Manifest text is required");
      return;
    }

    let epoch: number;
    if (expiryMode === "hours") {
      const hours = parseFloat(expiryHours);
      if (isNaN(hours) || hours <= 0) {
        setError("Hours must be greater than 0");
        return;
      }
      if (currentEpoch === undefined || secondsPerEpoch === undefined) {
        setError("Epoch data not loaded yet");
        return;
      }
      epoch = currentEpoch + hoursToEpochs(hours, secondsPerEpoch);
    } else {
      epoch = parseInt(expiryEpoch, 10);
      if (isNaN(epoch) || epoch <= 0) {
        setError("Expiry epoch must be a positive number");
        return;
      }
      if (currentEpoch !== undefined && epoch <= currentEpoch) {
        setError(
          `Expiry epoch must be greater than the current epoch (${currentEpoch})`
        );
        return;
      }
    }

    setSubmitting(true);
    try {
      const proposal = await createProposal({
        manifest_text: manifestText,
        expiry_epoch: epoch,
        multisig_account: multisigAccount.trim(),
      });
      navigate({ to: "/proposals/$id", params: { id: proposal.id } });
    } catch (err) {
      setError(String(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      <div className="space-y-2">
        <label htmlFor="multisig-account" className="text-sm font-medium">
          Multisig Account Address
        </label>
        <input
          id="multisig-account"
          type="text"
          value={multisigAccount}
          onChange={(e) => setMultisigAccount(e.target.value)}
          placeholder="account_tdx_2_..."
          className="w-full px-4 py-2 rounded-lg bg-muted border border-border font-mono text-sm focus:outline-none focus:ring-2 focus:ring-accent placeholder:text-muted-foreground/50"
          disabled={submitting}
        />
        <p className="text-xs text-muted-foreground">
          The multisig account that this proposal will execute against.
        </p>
      </div>

      <div className="space-y-2">
        <label htmlFor="manifest" className="text-sm font-medium">
          Transaction Manifest
        </label>
        <textarea
          id="manifest"
          value={manifestText}
          onChange={(e) => setManifestText(e.target.value)}
          placeholder={`CALL_METHOD\n    Address("account_tdx_2_...")\n    "withdraw"\n    ...\n;`}
          className="w-full h-64 px-4 py-3 rounded-lg bg-muted border border-border font-mono text-sm resize-y placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-accent"
          disabled={submitting}
        />
        <p className="text-xs text-muted-foreground">
          Raw RTM (Radix Transaction Manifest). YIELD_TO_PARENT will be appended
          automatically.
        </p>
      </div>

      <div className="space-y-2">
        <label className="text-sm font-medium">Expiry</label>
        <div className="flex gap-1">
          <button
            type="button"
            onClick={() => setExpiryMode("hours")}
            className={`px-3 py-1 text-xs rounded-md transition-colors ${
              expiryMode === "hours"
                ? "bg-accent text-white"
                : "bg-muted text-muted-foreground hover:text-foreground"
            }`}
          >
            Hours
          </button>
          <button
            type="button"
            onClick={() => setExpiryMode("epoch")}
            className={`px-3 py-1 text-xs rounded-md transition-colors ${
              expiryMode === "epoch"
                ? "bg-accent text-white"
                : "bg-muted text-muted-foreground hover:text-foreground"
            }`}
          >
            Exact Epoch
          </button>
        </div>

        {expiryMode === "hours" ? (
          <>
            <input
              id="expiry-hours"
              type="number"
              value={expiryHours}
              onChange={(e) => setExpiryHours(e.target.value)}
              placeholder="e.g. 2, 24, 168"
              className="w-48 px-4 py-2 rounded-lg bg-muted border border-border text-sm focus:outline-none focus:ring-2 focus:ring-accent"
              disabled={submitting}
              min="0"
              step="any"
            />
            <p className="text-xs text-muted-foreground">
              How many hours until this proposal expires.
            </p>
            {(() => {
              const hours = parseFloat(expiryHours);
              if (
                isNaN(hours) ||
                hours <= 0 ||
                currentEpoch === undefined ||
                secondsPerEpoch === undefined
              )
                return null;
              const epochsNeeded = hoursToEpochs(hours, secondsPerEpoch);
              const resolved = currentEpoch + epochsNeeded;
              return (
                <p className="text-xs text-muted-foreground">
                  Expires at epoch ~{resolved} (
                  {formatEpochDelta(epochsNeeded, secondsPerEpoch)} from now)
                </p>
              );
            })()}
          </>
        ) : (
          <>
            <input
              id="expiry-epoch"
              type="number"
              value={expiryEpoch}
              onChange={(e) => setExpiryEpoch(e.target.value)}
              placeholder={
                currentEpoch !== undefined
                  ? `e.g. ${currentEpoch + 100}`
                  : "e.g. 50000"
              }
              className="w-48 px-4 py-2 rounded-lg bg-muted border border-border text-sm focus:outline-none focus:ring-2 focus:ring-accent"
              disabled={submitting}
            />
            <p className="text-xs text-muted-foreground">
              The epoch at which this proposal expires. Must be greater than the
              current epoch.
            </p>
            {(() => {
              const epoch = parseInt(expiryEpoch, 10);
              if (
                isNaN(epoch) ||
                currentEpoch === undefined ||
                secondsPerEpoch === undefined ||
                epoch <= currentEpoch
              )
                return null;
              return (
                <p className="text-xs text-muted-foreground">
                  {formatEpochDelta(epoch - currentEpoch, secondsPerEpoch)} from
                  now
                </p>
              );
            })()}
          </>
        )}

        <p className="text-xs text-muted-foreground">
          {currentEpoch !== undefined
            ? `Current epoch: ${currentEpoch}`
            : Result.builder(epochDurationResult)
                .onInitial(() => "Loading current epoch...")
                .onFailure(() => "Failed to fetch current epoch")
                .orElse(() => "")}
        </p>
      </div>

      {error && (
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">
          {error}
        </div>
      )}

      <button
        type="submit"
        disabled={submitting}
        className="inline-flex items-center gap-2 rounded-md bg-accent px-6 py-2.5 text-sm font-medium text-white hover:bg-accent/80 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {submitting ? "Creating..." : "Create Proposal"}
      </button>
    </form>
  );
}
