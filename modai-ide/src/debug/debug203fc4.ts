const ENDPOINT = "http://127.0.0.1:7857/ingest/d61536e6-24d2-46a3-a36e-bb22726ddb0e";
const SESSION = "203fc4";

export function logDebug203(
  hypothesisId: string,
  location: string,
  message: string,
  data?: Record<string, unknown>,
  runId = "pre",
): void {
  fetch(ENDPOINT, {
    method: "POST",
    headers: { "Content-Type": "application/json", "X-Debug-Session-Id": SESSION },
    body: JSON.stringify({
      sessionId: SESSION,
      runId,
      hypothesisId,
      location,
      message,
      data: data ?? {},
      timestamp: Date.now(),
    }),
  }).catch(() => {});
}
