import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatPrice(val: string | number | null | undefined): string {
  if (val === null || val === undefined) return "—";
  const num = typeof val === "number" ? val : parseFloat(val);
  if (isNaN(num)) return String(val);
  if (Math.abs(num) >= 1000) {
    return num.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  } else if (Math.abs(num) >= 1) {
    return num.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 4 });
  } else if (Math.abs(num) > 0.0001) {
    return num.toLocaleString("en-US", { minimumFractionDigits: 4, maximumFractionDigits: 6 });
  } else {
    return num.toLocaleString("en-US", { minimumFractionDigits: 6, maximumFractionDigits: 8 });
  }
}

export function formatPercent(val: number | null | undefined): string {
  if (val === null || val === undefined || isNaN(val)) return "—";
  const sign = val > 0 ? "+" : "";
  return `${sign}${val.toFixed(2)}%`;
}

export function formatVolume(val: string | number | null | undefined): string {
  if (val === null || val === undefined) return "—";
  const num = typeof val === "number" ? val : parseFloat(val);
  if (isNaN(num)) return String(val);
  if (num >= 1_000_000_000) return `${(num / 1_000_000_000).toFixed(2)}B`;
  if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(2)}M`;
  if (num >= 1_000) return `${(num / 1_000).toFixed(2)}K`;
  return num.toFixed(2);
}
