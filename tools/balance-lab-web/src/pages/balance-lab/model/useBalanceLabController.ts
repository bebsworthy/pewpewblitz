import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyBalanceLabSnapshot,
  fetchBalanceLabState,
  restoreBalanceLabDefaults,
} from "../api/balanceLabApi";
import type { BalanceLabSnapshot, BalanceLabState, JsonValue } from "./balanceLab";

const clone = <T,>(value: T): T => structuredClone(value);

function replaceAtPath(value: JsonValue, path: (string | number)[], next: number): JsonValue {
  if (path.length === 0) return next;
  const [head, ...tail] = path;
  if (Array.isArray(value)) {
    const copy = [...value];
    copy[Number(head)] = replaceAtPath(copy[Number(head)], tail, next);
    return copy;
  }
  const copy = { ...(value as Record<string, JsonValue>) };
  copy[String(head)] = replaceAtPath(copy[String(head)], tail, next);
  return copy;
}

export function useBalanceLabController() {
  const [state, setState] = useState<BalanceLabState | null>(null);
  const [draft, setDraft] = useState<BalanceLabSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [connected, setConnected] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [lastTransaction, setLastTransaction] = useState<BalanceLabState["lastTransaction"]>(null);
  const synchronizedWorker = useRef<string | null>(null);
  const observedMatch = useRef<string | null>(null);
  const observedTransaction = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    try {
      const next = await fetchBalanceLabState();
      setState(next);
      setConnected(true);
      setError(null);
      if (observedMatch.current !== next.matchId) {
        observedMatch.current = next.matchId;
        observedTransaction.current = next.lastTransaction?.id ?? null;
        setLastTransaction(null);
      } else if (
        next.lastTransaction &&
        observedTransaction.current !== next.lastTransaction.id
      ) {
        observedTransaction.current = next.lastTransaction.id;
        setLastTransaction(next.lastTransaction);
      }
      const workerKey = `${next.matchId}:${next.revision}`;
      if (synchronizedWorker.current !== workerKey) {
        synchronizedWorker.current = workerKey;
        setDraft(clone(next.applied));
      }
    } catch (reason) {
      setConnected(false);
      setError(
        reason instanceof Error && reason.message !== "Failed to fetch"
          ? reason.message
          : "Waiting for an authoritative Practice worker…",
      );
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 500);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const setNumber = useCallback((path: (string | number)[], value: number) => {
    setError(null);
    setLastTransaction(null);
    setDraft((current) =>
      current ? (replaceAtPath(current, path, value) as BalanceLabSnapshot) : current,
    );
  }, []);

  const apply = useCallback(async () => {
    if (!state || !draft) return;
    setSubmitting(true);
    try {
      await applyBalanceLabSnapshot(state.revision, draft);
      setError(null);
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Apply failed");
    } finally {
      setSubmitting(false);
    }
  }, [draft, refresh, state]);

  const restore = useCallback(async () => {
    if (!state) return;
    setSubmitting(true);
    try {
      await restoreBalanceLabDefaults(state.revision);
      setError(null);
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Restore failed");
    } finally {
      setSubmitting(false);
    }
  }, [refresh, state]);

  const revert = useCallback(() => {
    if (state) {
      setError(null);
      setLastTransaction(null);
      setDraft(clone(state.applied));
    }
  }, [state]);

  const dirty = Boolean(state && draft && JSON.stringify(state.applied) !== JSON.stringify(draft));
  return {
    state,
    draft,
    error,
    lastTransaction,
    connected,
    dirty,
    submitting,
    setNumber,
    apply,
    restore,
    revert,
  };
}
