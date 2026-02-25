import { Duration } from "effect";

export function hoursToEpochs(hours: number, secondsPerEpoch: number): number {
  const totalSeconds = Duration.toSeconds(Duration.hours(hours));
  return Math.ceil(totalSeconds / secondsPerEpoch);
}

export function formatEpochDelta(
  epochsRemaining: number,
  secondsPerEpoch: number
): string {
  if (epochsRemaining <= 0) return "Expired";

  const totalSeconds = epochsRemaining * secondsPerEpoch;

  if (totalSeconds < 300) return "< 5 min";

  const minutes = totalSeconds / 60;
  if (minutes < 60) return `~${Math.round(minutes)} min`;

  const hours = minutes / 60;
  if (hours < 24) return `~${Math.round(hours)}h`;

  const days = hours / 24;
  return `~${Math.round(days)}d`;
}
