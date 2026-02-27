import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Result, useAtomSet, useAtomValue } from "@effect-atom/atom-react";
import { createProposalAtom } from "@/atom/proposalAtoms";
import { epochDurationAtom } from "@/atom/gatewayAtoms";
import { formatEpochDelta, hoursToEpochs } from "@/lib/epochTime";
import { ClientOnly } from "@/lib/ClientOnly";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { BackLink } from "@/components/back-link";

export const Route = createFileRoute("/proposals/new")({
  component: NewProposalPage,
});

function NewProposalPage() {
  return (
    <div className="space-y-6">
      <div>
        <BackLink />
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
      <Skeleton className="h-48 w-full" />
      <Skeleton className="h-10 w-32" />
    </div>
  );
}

function CreateProposalForm() {
  const navigate = useNavigate();
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
        <Label htmlFor="manifest">Transaction Manifest</Label>
        <Textarea
          id="manifest"
          value={manifestText}
          onChange={(e) => setManifestText(e.target.value)}
          placeholder={`CALL_METHOD\n    Address("account_tdx_2_...")\n    "withdraw"\n    ...\n;`}
          className="h-64 font-mono text-sm resize-y"
          disabled={submitting}
        />
        <p className="text-xs text-muted-foreground">
          Raw RTM (Radix Transaction Manifest). YIELD_TO_PARENT will be appended
          automatically.
        </p>
      </div>

      <div className="space-y-2">
        <Label>Expiry</Label>
        <div className="flex gap-1">
          <Button
            type="button"
            size="sm"
            variant={expiryMode === "hours" ? "accent" : "ghost"}
            onClick={() => setExpiryMode("hours")}
          >
            Hours
          </Button>
          <Button
            type="button"
            size="sm"
            variant={expiryMode === "epoch" ? "accent" : "ghost"}
            onClick={() => setExpiryMode("epoch")}
          >
            Exact Epoch
          </Button>
        </div>

        {expiryMode === "hours" ? (
          <>
            <Input
              id="expiry-hours"
              type="number"
              value={expiryHours}
              onChange={(e) => setExpiryHours(e.target.value)}
              placeholder="e.g. 2, 24, 168"
              className="w-48"
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
            <Input
              id="expiry-epoch"
              type="number"
              value={expiryEpoch}
              onChange={(e) => setExpiryEpoch(e.target.value)}
              placeholder={
                currentEpoch !== undefined
                  ? `e.g. ${currentEpoch + 100}`
                  : "e.g. 50000"
              }
              className="w-48"
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
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <Button type="submit" variant="accent" disabled={submitting}>
        {submitting ? "Creating..." : "Create Proposal"}
      </Button>
    </form>
  );
}
