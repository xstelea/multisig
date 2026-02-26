import { blake2b } from "@noble/hashes/blake2.js";

// --- Network constants ---

const NETWORK_CONSTANTS = {
  2: {
    signatureBadgeResource:
      "resource_tdx_2_1nfxxxxxxxxxxed25sgxxxxxxxxx002236757237xxxxxxxxx3e2cpa",
  },
  1: {
    signatureBadgeResource:
      "resource_rdx1nfxxxxxxxxxxed25sgxxxxxxxxx002236757237xxxxxxxxxed25sg",
  },
} as Record<number, { signatureBadgeResource: string } | undefined>;

export function getNetworkConstants(networkId: number) {
  const constants = NETWORK_CONSTANTS[networkId];
  if (!constants) throw new Error(`Unsupported network ID: ${networkId}`);
  return constants;
}

// --- Hex utilities ---

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

// --- Local ID derivation ---

export function deriveLocalId(pubkeyHex: string): string {
  const bytes = hexToBytes(pubkeyHex);
  const hash = blake2b(bytes, { dkLen: 32 });
  return bytesToHex(hash.slice(-26));
}

// --- Signer parsing ---

export type ParsedSigner = {
  resource: string;
  localId: string;
};

const HEX_64_RE = /^[0-9a-fA-F]{64}$/;
const BADGE_RE = /^(resource_[a-z0-9_]+):\[([0-9a-fA-F]+)\]$/;

export function parseSigner(
  input: string,
  networkId: number
): ParsedSigner | { error: string } {
  const trimmed = input.trim();
  if (!trimmed) return { error: "Empty input" };

  if (HEX_64_RE.test(trimmed)) {
    const { signatureBadgeResource } = getNetworkConstants(networkId);
    return {
      resource: signatureBadgeResource,
      localId: deriveLocalId(trimmed),
    };
  }

  const match = BADGE_RE.exec(trimmed);
  if (match) {
    return { resource: match[1], localId: match[2] };
  }

  return {
    error:
      "Expected 64 hex chars (Ed25519 pubkey) or resource_...:[] (badge ID)",
  };
}

// --- Manifest builder ---

export function buildCreateAccountManifest({
  signers,
  threshold,
}: {
  signers: ParsedSigner[];
  threshold: number;
}): string {
  const signerEntries = signers
    .map(
      (s) =>
        `                        Enum<0u8>(\n` +
        `                            NonFungibleGlobalId("${s.resource}:[${s.localId}]")\n` +
        `                        )`
    )
    .join(",\n");

  return `CREATE_ACCOUNT_ADVANCED
    Enum<1u8>(
        Enum<2u8>(
            Enum<0u8>(
                Enum<2u8>(
                    ${threshold}u8,
                    Array<Enum>(
${signerEntries}
                    )
                )
            )
        )
    )
    Enum<0u8>()
;`;
}
