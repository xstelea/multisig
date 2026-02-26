import { Badge } from "@/components/ui/badge";

const STATUS_VARIANTS: Record<string, { className: string }> = {
  created: { className: "bg-blue-500/20 text-blue-400 hover:bg-blue-500/20" },
  signing: {
    className: "bg-yellow-500/20 text-yellow-400 hover:bg-yellow-500/20",
  },
  ready: { className: "bg-green-500/20 text-green-400 hover:bg-green-500/20" },
  submitting: {
    className: "bg-purple-500/20 text-purple-400 hover:bg-purple-500/20",
  },
  committed: {
    className: "bg-emerald-500/20 text-emerald-400 hover:bg-emerald-500/20",
  },
  failed: { className: "bg-red-500/20 text-red-400 hover:bg-red-500/20" },
  expired: { className: "bg-gray-500/20 text-gray-400 hover:bg-gray-500/20" },
  invalid: { className: "bg-red-500/20 text-red-400 hover:bg-red-500/20" },
};

const DEFAULT_VARIANT = {
  className: "bg-gray-500/20 text-gray-400 hover:bg-gray-500/20",
};

export function StatusBadge({ status }: { status: string }) {
  const { className } = STATUS_VARIANTS[status] ?? DEFAULT_VARIANT;
  return (
    <Badge variant="outline" className={`border-0 ${className}`}>
      {status}
    </Badge>
  );
}
