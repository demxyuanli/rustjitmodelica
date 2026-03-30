export const DAILY_TOKEN_LIMIT = 50000;

export function estimateTokens(text: string): number {
  return Math.ceil(text.length * 1.2);
}

export function getDailyUsed(): number {
  try {
    const raw = localStorage.getItem("modai-ai-daily");
    if (!raw) return 0;
    const { date, used } = JSON.parse(raw) as { date: string; used: number };
    const today = new Date().toISOString().slice(0, 10);
    return date === today ? used : 0;
  } catch {
    return 0;
  }
}

export function setDailyUsedStorage(used: number): void {
  try {
    const date = new Date().toISOString().slice(0, 10);
    localStorage.setItem("modai-ai-daily", JSON.stringify({ date, used }));
  } catch {
    /* ignore */
  }
}
