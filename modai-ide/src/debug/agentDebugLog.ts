// #region agent log
// Opt-in: set VITE_MODAI_AGENT_DEBUG_LOG=1 and run a local ingest (e.g. on port 7857).
// Default off to avoid net::ERR_CONNECTION_REFUSED in the devtools console and wasted requests.
const DEBUG_INGEST =
  "http://127.0.0.1:7857/ingest/d61536e6-24d2-46a3-a36e-bb22726ddb0e";
const DEBUG_SESSION = "a73e53";

const INGEST_ENABLED =
  import.meta.env.VITE_MODAI_AGENT_DEBUG_LOG === "1" ||
  import.meta.env.VITE_MODAI_AGENT_DEBUG_LOG === "true";

export function agentDebugLog(payload: {
  location: string;
  message: string;
  data?: Record<string, unknown>;
  hypothesisId: string;
  runId?: string;
}): void {
  if (!INGEST_ENABLED) return;
  void fetch(DEBUG_INGEST, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Debug-Session-Id": DEBUG_SESSION,
    },
    body: JSON.stringify({
      sessionId: DEBUG_SESSION,
      timestamp: Date.now(),
      runId: payload.runId ?? "pre-fix",
      ...payload,
    }),
  }).catch(() => {});
}
// #endregion
