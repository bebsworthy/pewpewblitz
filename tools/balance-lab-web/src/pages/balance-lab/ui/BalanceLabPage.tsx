import { useBalanceLabController } from "../model/useBalanceLabController";
import { BalanceLabToast } from "./BalanceLabToast";
import { EditorWorkspace } from "./EditorWorkspace";
import { PlayerLoadouts } from "./PlayerLoadouts";

export function BalanceLabPage() {
  const controller = useBalanceLabController();
  const { state, draft } = controller;
  if (!state || !draft) {
    return (
      <main className="loading-shell">
        <p className="eyebrow">PewPew Blitz · Development tools</p>
        <h1>Balance Lab</h1>
        <p>{controller.error ?? "Connecting to the authoritative Practice worker…"}</p>
      </main>
    );
  }

  const busy = !controller.connected || controller.submitting || Boolean(state.pending);
  const status = !controller.connected
    ? "Waiting for Practice"
    : busy
      ? state.pending?.message ?? "Applying…"
      : controller.hasFieldErrors
        ? "Draft needs attention"
        : controller.dirty
          ? `${controller.changedCount} field${controller.changedCount === 1 ? "" : "s"} changed`
          : "Authoritative";

  return (
    <main>
      <header className="topbar">
        <div>
          <p className="eyebrow">PewPew Blitz · Development tools</p>
          <h1>Balance Lab</h1>
          <p>Match {state.matchId} · Applied revision {state.revision}</p>
        </div>
        <div
          className={`status ${!controller.connected ? "offline" : busy ? "pending" : controller.hasFieldErrors ? "error" : "ready"}`}
        >
          {status}
        </div>
      </header>

      {(controller.error || controller.lastTransaction) && (
        <BalanceLabToast
          key={controller.error ?? controller.lastTransaction?.id}
          message={controller.error ?? controller.lastTransaction?.message ?? ""}
          error={Boolean(
            controller.error || controller.lastTransaction?.status === "rejected",
          )}
        />
      )}

      <PlayerLoadouts players={state.players} />

      <EditorWorkspace
        fields={state.editorManifest.fields}
        draft={draft}
        applied={state.applied}
        baseline={state.baseline}
        errors={controller.fieldErrors}
        disabled={busy}
        onChange={controller.setFieldText}
        onReset={controller.resetField}
      />

      <footer className="actionbar">
        <div>
          <strong>
            {controller.hasFieldErrors
              ? "Fix invalid fields before applying"
              : controller.dirty
                ? `${controller.changedCount} unapplied change${controller.changedCount === 1 ? "" : "s"}`
                : "Draft matches the server"}
          </strong>
          <small>Apply starts a clean, authoritative Practice epoch.</small>
        </div>
        <div className="actions">
          <button type="button" disabled={busy || !controller.dirty} onClick={controller.revert}>
            Revert draft
          </button>
          <button type="button" disabled={busy} onClick={() => void controller.restore()}>
            Restore canonical defaults
          </button>
          <button
            className="primary"
            type="button"
            disabled={busy || !controller.dirty || controller.hasFieldErrors}
            onClick={() => void controller.apply()}
          >
            Apply &amp; reset match
          </button>
        </div>
      </footer>
    </main>
  );
}
