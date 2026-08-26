import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyBalanceLabSnapshot,
  fetchBalanceLabState,
  restoreBalanceLabDefaults,
} from "../api/balanceLabApi";
import {
  changedFields,
  fieldFromServerError,
  pathKey,
  replaceAtPath,
  storedNumber,
  toStoredNumber,
  validateDisplayNumber,
} from "../lib/editorFields";
import type {
  BalanceLabSnapshot,
  BalanceLabState,
  EditorFieldDescriptor,
} from "./balanceLab";

const clone = <T,>(value: T): T => structuredClone(value);

export function useBalanceLabController() {
  const [state, setState] = useState<BalanceLabState | null>(null);
  const [draft, setDraft] = useState<BalanceLabSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [connected, setConnected] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
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
        if (next.lastTransaction.status === "rejected") {
          const identified = fieldFromServerError(
            next.lastTransaction.message,
            next.editorManifest.fields,
          );
          if (identified) {
            setFieldErrors((current) => ({
              ...current,
              [pathKey(identified.field.path)]: identified.message,
            }));
          }
        }
      }
      const workerKey = `${next.matchId}:${next.revision}`;
      if (synchronizedWorker.current !== workerKey) {
        synchronizedWorker.current = workerKey;
        setDraft(clone(next.applied));
        setFieldErrors({});
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

  const setFieldText = useCallback((field: EditorFieldDescriptor, text: string) => {
    const key = pathKey(field.path);
    const display = Number(text);
    const validation = text.trim() === "" ? "Enter a number." : validateDisplayNumber(display, field);
    setFieldErrors((current) => {
      const next = { ...current };
      if (validation) next[key] = validation;
      else delete next[key];
      return next;
    });
    if (validation) return;
    setError(null);
    setLastTransaction(null);
    setDraft((current) =>
      current
        ? (replaceAtPath(
            current,
            field.path,
            toStoredNumber(display, field),
          ) as BalanceLabSnapshot)
        : current,
    );
  }, []);

  const resetField = useCallback(
    (field: EditorFieldDescriptor) => {
      if (!state) return;
      const key = pathKey(field.path);
      setFieldErrors((current) => {
        const next = { ...current };
        delete next[key];
        return next;
      });
      setDraft((current) =>
        current
          ? (replaceAtPath(
              current,
              field.path,
              storedNumber(state.applied, field),
            ) as BalanceLabSnapshot)
          : current,
      );
    },
    [state],
  );

  const apply = useCallback(async () => {
    if (!state || !draft || Object.keys(fieldErrors).length > 0) return;
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
  }, [draft, fieldErrors, refresh, state]);

  const restore = useCallback(async () => {
    if (!state) return;
    setSubmitting(true);
    try {
      await restoreBalanceLabDefaults(state.revision, state.schemaVersion);
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
      setFieldErrors({});
    }
  }, [state]);

  const changed = state && draft
    ? changedFields(state.editorManifest.fields, draft, state.applied)
    : [];
  const dirty = changed.length > 0;
  return {
    state,
    draft,
    error,
    lastTransaction,
    connected,
    dirty,
    changedCount: changed.length,
    fieldErrors,
    hasFieldErrors: Object.keys(fieldErrors).length > 0,
    submitting,
    setFieldText,
    resetField,
    apply,
    restore,
    revert,
  };
}
