import { createFileRoute, Link } from "@tanstack/react-router";
import { useAtomMount, useAtomValue, Result } from "@effect-atom/atom-react";
import { dappToolkitAtom } from "@/atom/accessRuleAtom";
import { envVars, dashboardBaseUrl, gatewayBaseUrl } from "@/lib/envVars";
import { ClientOnly } from "@/lib/ClientOnly";
import {
  parseSigner,
  buildCreateAccountManifest,
  type ParsedSigner,
} from "@/lib/manifest";
import { useState, useMemo } from "react";
import { toast } from "sonner";
import * as Schema from "effect/Schema";
import * as Either from "effect/Either";

const ThresholdSchema = (max: number) =>
  Schema.NumberFromString.pipe(Schema.int(), Schema.between(1, max));

export const Route = createFileRoute("/create-account")({
  component: CreateAccountPage,
});

function CreateAccountPage() {
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
          Create Multisig Account
        </h1>
        <p className="text-muted-foreground mt-1">
          Define signers and threshold to create a new multisig account on Radix
          via your wallet.
        </p>
      </div>

      <ClientOnly fallback={<FormSkeleton />}>
        {() => <CreateAccountForm />}
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

async function fetchCreatedAccount(
  txId: string,
  networkId: number
): Promise<string | null> {
  try {
    const res = await fetch(
      `${gatewayBaseUrl(networkId)}/transaction/committed-details`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          intent_hash: txId,
          opt_ins: { affected_global_entities: true },
        }),
      }
    );
    if (!res.ok) return null;
    const data = await res.json();
    const entities: string[] =
      data?.transaction?.affected_global_entities ?? [];
    return entities.find((e) => e.startsWith("account_")) ?? null;
  } catch {
    return null;
  }
}

function CreateAccountForm() {
  useAtomMount(dappToolkitAtom);
  const rdtResult = useAtomValue(dappToolkitAtom);

  const [signers, setSigners] = useState<string[]>(["", ""]);
  const [thresholdInput, setThresholdInput] = useState("1");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [result, setResult] = useState<{
    txId: string;
    accountAddress: string | null;
  } | null>(null);

  const networkId = envVars.NETWORK_ID;

  // Parse all signers and determine which are valid
  const parsed = useMemo(
    () => signers.map((s) => (s.trim() ? parseSigner(s, networkId) : null)),
    [signers, networkId]
  );

  const validSigners = parsed.filter(
    (p): p is ParsedSigner => p !== null && !("error" in p)
  );
  const allFilled = signers.every((s) => s.trim() !== "");
  const allValid = allFilled && validSigners.length === signers.length;

  // Decode threshold from raw string input using Effect Schema
  const thresholdDecoded = useMemo(
    () =>
      Schema.decodeUnknownEither(ThresholdSchema(signers.length))(
        thresholdInput
      ),
    [thresholdInput, signers.length]
  );

  const effectiveThreshold = Either.isRight(thresholdDecoded)
    ? thresholdDecoded.right
    : 1;
  const thresholdError = Either.isLeft(thresholdDecoded)
    ? thresholdInput === ""
      ? "Required"
      : "Must be a whole number between 1 and " + signers.length
    : null;

  // Build manifest preview when all signers are valid and threshold is valid
  const thresholdValid = Either.isRight(thresholdDecoded);
  const manifestPreview = useMemo(() => {
    if (!allValid || validSigners.length === 0 || !thresholdValid) return null;
    return buildCreateAccountManifest({
      signers: validSigners,
      threshold: effectiveThreshold,
    });
  }, [allValid, validSigners, effectiveThreshold, thresholdValid]);

  const addSigner = () => setSigners((prev) => [...prev, ""]);
  const removeSigner = (index: number) => {
    setSigners((prev) => prev.filter((_, i) => i !== index));
    const newCount = signers.length - 1;
    const currentThreshold = Number(thresholdInput);
    if (!Number.isNaN(currentThreshold) && currentThreshold > newCount) {
      setThresholdInput(String(Math.max(1, newCount)));
    }
  };
  const updateSigner = (index: number, value: string) =>
    setSigners((prev) => prev.map((s, i) => (i === index ? value : s)));

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!allValid) {
      setError("All signer fields must contain valid keys");
      return;
    }
    if (!thresholdValid) {
      setError(
        "Threshold must be a valid number between 1 and " + signers.length
      );
      return;
    }
    if (!manifestPreview) {
      setError("Cannot build manifest");
      return;
    }

    const rdt = Result.builder(rdtResult)
      .onSuccess((v) => v)
      .orElse(() => null);
    if (!rdt) {
      setError("Radix wallet not connected");
      return;
    }

    setSubmitting(true);
    try {
      const response = await rdt.walletApi.sendTransaction({
        transactionManifest: manifestPreview,
        version: 2,
      });

      if (response.isErr()) {
        setError(response.error.message ?? "Transaction failed");
        return;
      }

      const txId = response.value.transactionIntentHash;
      const accountAddress = await fetchCreatedAccount(txId, networkId);
      setResult({ txId, accountAddress });
      toast.success("Account created!");
    } catch (err) {
      setError(String(err));
    } finally {
      setSubmitting(false);
    }
  };

  if (result) {
    const txDashboardUrl = `${dashboardBaseUrl(networkId)}/transaction/${result.txId}`;
    const accountDashboardUrl = result.accountAddress
      ? `${dashboardBaseUrl(networkId)}/account/${result.accountAddress}`
      : null;
    return (
      <div className="space-y-4">
        <div className="rounded-lg bg-green-500/10 border border-green-500/20 px-4 py-3 space-y-3">
          <p className="text-sm font-medium text-green-400">
            Account created successfully!
          </p>
          {result.accountAddress && (
            <div>
              <p className="text-xs text-muted-foreground">Account</p>
              <p className="text-sm font-mono break-all mt-0.5">
                {result.accountAddress}
              </p>
              <a
                href={accountDashboardUrl!}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-block mt-1 text-sm text-accent hover:underline"
              >
                View account on Dashboard &rarr;
              </a>
            </div>
          )}
          <div>
            <p className="text-xs text-muted-foreground">Transaction</p>
            <p className="text-xs font-mono break-all text-muted-foreground mt-0.5">
              {result.txId}
            </p>
            <a
              href={txDashboardUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-block mt-1 text-xs text-accent hover:underline"
            >
              View transaction &rarr;
            </a>
          </div>
        </div>
        <button
          type="button"
          onClick={() => {
            setResult(null);
            setSigners(["", ""]);
            setThresholdInput("1");
          }}
          className="text-sm text-muted-foreground hover:text-foreground transition-colors"
        >
          Create another account
        </button>
      </div>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      {/* Signer list */}
      <div className="space-y-2">
        <label className="text-sm font-medium">Signers</label>
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
                {signers.length > 2 && (
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
        <label htmlFor="threshold" className="text-sm font-medium">
          Threshold
        </label>
        <div className="flex items-center gap-3">
          <input
            id="threshold"
            type="text"
            inputMode="numeric"
            value={thresholdInput}
            onChange={(e) => setThresholdInput(e.target.value)}
            className={`w-20 px-3 py-2 rounded-lg bg-muted border text-sm focus:outline-none focus:ring-2 focus:ring-accent ${
              thresholdError ? "border-red-500/40" : "border-border"
            }`}
            disabled={submitting}
          />
          <span className="text-sm text-muted-foreground">
            of {signers.length} signer{signers.length !== 1 ? "s" : ""} required
          </span>
        </div>
        {thresholdError && (
          <p className="text-xs text-red-400">{thresholdError}</p>
        )}
        <p className="text-xs text-muted-foreground">
          Minimum signatures needed to authorize transactions.
        </p>
      </div>

      {/* Manifest preview */}
      {manifestPreview && (
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <label className="text-sm font-medium">Manifest Preview</label>
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

      {error && (
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400">
          {error}
        </div>
      )}

      <button
        type="submit"
        disabled={submitting || !allValid || !thresholdValid}
        className="inline-flex items-center gap-2 rounded-md bg-accent px-6 py-2.5 text-sm font-medium text-white hover:bg-accent/80 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {submitting ? "Creating..." : "Create Account"}
      </button>
    </form>
  );
}
