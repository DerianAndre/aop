/**
 * Formats a token count with K/M suffixes.
 *
 * @param count - The number to format
 * @returns Formatted string (e.g., '1.5K', '2.0M')
 *
 * @example
 * formatTokenCount(0)        // '0'
 * formatTokenCount(500)      // '500'
 * formatTokenCount(1500)     // '1.5K'
 * formatTokenCount(2000000)  // '2.0M'
 * formatTokenCount(-1500)    // '-1.5K'
 */
export function formatTokenCount(count: number): string {
  if (count === 0) return '0';

  const abs = Math.abs(count);
  const sign = count < 0 ? '-' : '';

  if (abs >= 1_000_000) {
    return `${sign}${(abs / 1_000_000).toFixed(1)}M`;
  }

  if (abs >= 1_000) {
    return `${sign}${(abs / 1_000).toFixed(1)}K`;
  }

  return `${sign}${abs}`;
}