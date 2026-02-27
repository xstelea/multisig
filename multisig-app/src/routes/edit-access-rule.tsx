import { createFileRoute, useNavigate } from "@tanstack/react-router";
import {
  Result,
  useAtomMount,
  useAtomSet,
  useAtomValue,
} from "@effect-atom/atom-react";
import { dappToolkitAtom } from "@/atom/accessRuleAtom";
import { createProposalAtom, getAccessRuleAtom } from "@/atom/proposalAtoms";
import { epochDurationAtom } from "@/atom/gatewayAtoms";
import { formatEpochDelta, hoursToEpochs } from "@/lib/epochTime";
import { envVars } from "@/lib/envVars";
import {
  parseSigner,
  buildSetOwnerRoleManifest,
  type ParsedSigner,
} from "@/lib/manifest";
import type { AccessRuleInfo } from "@/atom/orchestratorClient";
import { ClientOnly } from "@/lib/ClientOnly";
import { useState, useMemo, useEffect, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { BackLink } from "@/components/back-link";
import { toast } from "sonner";

type EditAccessRuleSearch = {
  account?: string;
};

export const Route = createFileRoute("/edit-access-rule")({
  validateSearch: (search: Record<string, unknown>): EditAccessRuleSearch => ({
    account:
      typeof search.account === "string" && search.account
        ? search.account
        : undefined,
  }),
  component: EditAccessRulePage,
});

function EditAccessRulePage() {
  return (
    <div className="space-y-6">
      <div>
        <BackLink />
        <h1 className="text-2xl font-semibold tracking-tight mt-2">
          Edit Access Rule
        </h1>
        <p className="text-muted-foreground mt-1">
          Change the signers and threshold for an existing multisig account.
        </p>
      </div>

      <ClientOnly fallback={<FormSkeleton />}>
        {() => <EditAccessRuleForm />}
      </ClientOnly>
    </div>
  );
}

function FormSkeleton() {
  return (
    <div className="space-y-4">
      <Skeleton className="h-10 w-full" />
      <Skeleton className="h-48 w-full" />
      <Skeleton className="h-10 w-32" />
    </div>
  );
}

function EditAccessRuleForm() {
  useAtomMount(dappToolkitAtom);
  const navigate = useNavigate();
  const { account: accountParam } = Route.useSearch();

  const networkId = envVars.NETWORK_ID;

  // Account address input
  const [accountAddress, setAccountAddress] = useState(accountParam ?? "");
  const [currentRule, setCurrentRule] = useState<AccessRuleInfo | null>(null);
  const [loadingRule, setLoadingRule] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  // Signer editing
  const [signers, setSigners] = useState<string[]>([""]);
  const [threshold, setThreshold] = useState(1);

  // Expiry
  const [expiryMode, setExpiryMode] = useState<"hours" | "epoch">("hours");
  const [expiryHours, setExpiryHours] = useState("");
  const [expiryEpoch, setExpiryEpoch] = useState("");

  // Submission
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const epochDurationResult = useAtomValue(epochDurationAtom);
  const epochDuration = Result.builder(epochDurationResult)
    .onSuccess((v) => v)
    .orElse(() => undefined);
  const currentEpoch = epochDuration?.currentEpoch;
  const secondsPerEpoch = epochDuration?.secondsPerEpoch;

  const getAccessRule = useAtomSet(getAccessRuleAtom, { mode: "promise" });
  const createProposal = useAtomSet(createProposalAtom, { mode: "promise" });

  const fetchAccessRule = useCallback(
    async (address: string) => {
      if (!address.trim()) return;
      setLoadingRule(true);
      setLoadError(null);
      setCurrentRule(null);
      try {
        const rule = await getAccessRule(address.trim());
        setCurrentRule(rule);

        // Pre-fill signers from current rule
        if (rule.signers.length > 0) {
          setSigners(
            rule.signers.map((s) => `${s.badge_resource}:${s.badge_local_id}`)
          );
          setThreshold(rule.threshold);
        }
      } catch (err) {
        setLoadError(String(err));
      } finally {
        setLoadingRule(false);
      }
    },
    [getAccessRule]
  );

  // Auto-fetch when account param is provided
  useEffect(() => {
    if (accountParam) {
      fetchAccessRule(accountParam);
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Parse all signers
  const parsed = useMemo(
    () => signers.map((s) => (s.trim() ? parseSigner(s, networkId) : null)),
    [signers, networkId]
  );

  const validSigners = parsed.filter(
    (p): p is ParsedSigner => p !== null && !("error" in p)
  );
  const allFilled = signers.every((s) => s.trim() !== "");
  const allValid = allFilled && validSigners.length === signers.length;

  const effectiveThreshold = Math.min(
    Math.max(1, threshold),
    validSigners.length || 1
  );

  // Build manifest preview
  const manifestPreview = useMemo(() => {
    if (!allValid || validSigners.length === 0 || !accountAddress.trim())
      return null;
    return buildSetOwnerRoleManifest({
      account: accountAddress.trim(),
      signers: validSigners,
      threshold: effectiveThreshold,
    });
  }, [allValid, validSigners, effectiveThreshold, accountAddress]);

  const addSigner = () => setSigners((prev) => [...prev, ""]);
  const removeSigner = (index: number) => {
    setSigners((prev) => prev.filter((_, i) => i !== index));
    if (threshold > signers.length - 1) {
      setThreshold(Math.max(1, signers.length - 1));
    }
  };
  const updateSigner = (index: number, value: string) =>
    setSigners((prev) => prev.map((s, i) => (i === index ? value : s)));

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!accountAddress.trim()) {
      setError("Account address is required");
      return;
    }
    if (!allValid || validSigners.length === 0) {
      setError("All signer fields must contain valid keys");
      return;
    }
    if (!manifestPreview) {
      setError("Cannot build manifest");
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
        manifest_text: manifestPreview,
        expiry_epoch: epoch,
        multisig_account: accountAddress.trim(),
      });
      toast.success("Proposal created!");
      navigate({ to: "/proposals/$id", params: { id: proposal.id } });
    } catch (err) {
      setError(String(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      {/* Account address */}
      <div className="space-y-2">
        <Label htmlFor="account-address">Multisig Account Address</Label>
        <div className="flex gap-2">
          <Input
            id="account-address"
            value={accountAddress}
            onChange={(e) => setAccountAddress(e.target.value)}
            placeholder="account_tdx_2_..."
            className="font-mono text-sm"
            disabled={loadingRule || submitting}
          />
          <Button
            type="button"
            variant="outline"
            onClick={() => fetchAccessRule(accountAddress)}
            disabled={!accountAddress.trim() || loadingRule}
          >
            {loadingRule ? "Loading..." : "Load"}
          </Button>
        </div>
        <p className="text-xs text-muted-foreground">
          Enter the address of the multisig account whose access rule you want
          to change.
        </p>
      </div>

      {loadError && (
        <Alert variant="destructive">
          <AlertDescription>{loadError}</AlertDescription>
        </Alert>
      )}

      {/* Current access rule display */}
      {currentRule && (
        <div className="rounded-lg border border-border bg-muted/30 px-4 py-3 space-y-2">
          <p className="text-sm font-medium">Current Access Rule</p>
          <p className="text-xs text-muted-foreground">
            Threshold: {currentRule.threshold} of {currentRule.signers.length}{" "}
            signer{currentRule.signers.length !== 1 ? "s" : ""}
          </p>
          <div className="space-y-1">
            {currentRule.signers.map((s, i) => (
              <p
                key={i}
                className="text-xs font-mono text-muted-foreground truncate"
              >
                {s.badge_resource}:{s.badge_local_id}
              </p>
            ))}
          </div>
        </div>
      )}

      {/* Signer list (only show after rule is loaded) */}
      {currentRule && (
        <>
          <div className="space-y-2">
            <Label>New Signers</Label>
            <p className="text-xs text-muted-foreground">
              Enter Ed25519 public keys (64 hex chars) or badge IDs
              (resource_...:&#91;...&#93;).
            </p>

            <div className="space-y-2">
              {signers.map((signer, i) => {
                const p = parsed[i];
                const hasInput = signer.trim() !== "";
                const isValid = p !== null && !("error" in p);
                const errorMsg =
                  hasInput && p !== null && "error" in p ? p.error : null;

                return (
                  <div key={i} className="flex items-start gap-2">
                    <div className="flex-1 space-y-1">
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground w-6">
                          #{i + 1}
                        </span>
                        <input
                          type="text"
                          value={signer}
                          onChange={(e) => updateSigner(i, e.target.value)}
                          placeholder="Ed25519 public key or badge ID"
                          className={`flex-1 px-3 py-2 rounded-lg bg-muted border text-sm font-mono focus:outline-none focus:ring-2 focus:ring-accent ${
                            hasInput
                              ? isValid
                                ? "border-green-500/40"
                                : "border-red-500/40"
                              : "border-border"
                          }`}
                          disabled={submitting}
                        />
                        {hasInput && (
                          <span
                            className={`text-xs ${isValid ? "text-green-400" : "text-red-400"}`}
                          >
                            {isValid ? "valid" : "invalid"}
                          </span>
                        )}
                      </div>
                      {errorMsg && (
                        <p className="text-xs text-red-400 ml-8">{errorMsg}</p>
                      )}
                    </div>
                    {signers.length > 1 && (
                      <button
                        type="button"
                        onClick={() => removeSigner(i)}
                        className="mt-2 text-muted-foreground hover:text-red-400 transition-colors"
                        title="Remove signer"
                      >
                        <svg
                          xmlns="http://www.w3.org/2000/svg"
                          width="16"
                          height="16"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="2"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                        >
                          <path d="M18 6 6 18" />
                          <path d="m6 6 12 12" />
                        </svg>
                      </button>
                    )}
                  </div>
                );
              })}
            </div>

            <button
              type="button"
              onClick={addSigner}
              className="text-sm text-accent hover:text-accent/80 transition-colors"
            >
              + Add signer
            </button>
          </div>

          {/* Threshold */}
          <div className="space-y-2">
            <Label htmlFor="threshold">Threshold</Label>
            <div className="flex items-center gap-3">
              <Input
                id="threshold"
                type="number"
                value={threshold}
                onChange={(e) =>
                  setThreshold(parseInt(e.target.value, 10) || 1)
                }
                min={1}
                max={signers.length}
                className="w-20"
                disabled={submitting}
              />
              <span className="text-sm text-muted-foreground">
                of {signers.length} signer
                {signers.length !== 1 ? "s" : ""} required
              </span>
            </div>
          </div>

          {/* Manifest preview */}
          {manifestPreview && (
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <Label>Manifest Preview</Label>
                <button
                  type="button"
                  onClick={() => {
                    navigator.clipboard.writeText(manifestPreview);
                    toast.success("Manifest copied");
                  }}
                  className="text-xs text-muted-foreground hover:text-foreground transition-colors"
                >
                  Copy
                </button>
              </div>
              <pre className="px-4 py-3 rounded-lg bg-muted border border-border font-mono text-xs overflow-x-auto max-h-64 overflow-y-auto">
                {manifestPreview}
              </pre>
            </div>
          )}

          {/* Expiry */}
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
                      {formatEpochDelta(epochsNeeded, secondsPerEpoch)} from
                      now)
                    </p>
                  );
                })()}
              </>
            ) : (
              <>
                <Input
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
                  The epoch at which this proposal expires.
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
                      {formatEpochDelta(epoch - currentEpoch, secondsPerEpoch)}{" "}
                      from now
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

          <Button
            type="submit"
            variant="accent"
            disabled={submitting || !allValid}
          >
            {submitting ? "Creating Proposal..." : "Create Proposal"}
          </Button>
        </>
      )}
    </form>
  );
}
