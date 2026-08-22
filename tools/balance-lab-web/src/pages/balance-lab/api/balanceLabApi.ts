import type { BalanceLabSnapshot, BalanceLabState } from "../model/balanceLab";

async function checked(response: Response) {
  if (!response.ok) {
    throw new Error((await response.text()) || `Request failed (${response.status})`);
  }
  return response;
}
export async function fetchBalanceLabState(): Promise<BalanceLabState> {
  return (await checked(await fetch("/api/v1/state", { cache: "no-store" }))).json();
}

export async function applyBalanceLabSnapshot(
  revision: number,
  snapshot: BalanceLabSnapshot,
) {
  await checked(
    await fetch("/api/v1/apply", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ schemaVersion: 1, expectedRevision: revision, snapshot }),
    }),
  );
}

export async function restoreBalanceLabDefaults(revision: number) {
  await checked(
    await fetch("/api/v1/restore-defaults", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ schemaVersion: 1, expectedRevision: revision }),
    }),
  );
}
