import { useBalanceLabController } from "../model/useBalanceLabController";
import { BalanceLabToast } from "./BalanceLabToast";
import { NumericTreeEditor } from "./NumericTreeEditor";
import { WeaponSummary } from "./WeaponSummary";

export function BalanceLabPage() {
  const controller = useBalanceLabController();
  const { state, draft } = controller;
  if (!state || !draft) {
    return (
      <main className="loading-shell">
        <h1>Balance Lab</h1>
        <p>{controller.error ?? "Connecting to the authoritative Practice worker…"}</p>
      </main>
    );
  }
  const busy = !controller.connected || controller.submitting || Boolean(state.pending);
  return (
    <main>
      <header className="topbar">
        <div>
          <p className="eyebrow">PewPew Blitz · V10</p>
          <h1>Balance Lab</h1>
          <p>Match {state.matchId} · Applied revision {state.revision}</p>
        </div>
        <div className={`status ${!controller.connected ? "offline" : busy ? "pending" : "ready"}`}>
          {!controller.connected
            ? "Waiting for Practice"
            : busy
              ? state.pending?.message
              : controller.dirty
                ? "Draft changed"
                : "Authoritative"}
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

      <section className="panel">
        <h2>Fighter profiles</h2>
        <p>Permanent creation profiles used by every saved brawler.</p>
        <NumericTreeEditor
          value={draft.fighterProfiles}
          path={["fighterProfiles"]}
          onNumber={controller.setNumber}
        />
      </section>

      <section>
        <div className="section-heading">
          <div>
            <h2>Weapon recipes</h2>
            <p>Recipe structure is locked; numeric leaves remain server-validated.</p>
          </div>
        </div>
        <div className="weapon-grid">
          {draft.weapons.map((weapon, index) => (
            <article className="panel weapon" key={weapon.id}>
              <div className="weapon-heading">
                <div>
                  <p className="eyebrow">Weapon base {weapon.id}</p>
                  <h3>{weapon.displayName}</h3>
                </div>
                <code>{weapon.key}</code>
              </div>
              <WeaponSummary weapon={weapon} />
              <NumericTreeEditor
                value={weapon.recipe}
                path={["weapons", index, "recipe"]}
                onNumber={controller.setNumber}
              />
            </article>
          ))}
        </div>
      </section>

      <section>
        <div className="section-heading">
          <div>
            <h2>Concealment ultimates</h2>
            <p>Durations, ranges, and radii for the V9 concealment and counter-play families.</p>
          </div>
        </div>
        <div className="weapon-grid">
          {draft.ultimates.map((ultimate, index) => (
            <article className="panel weapon" key={ultimate.id}>
              <div className="weapon-heading">
                <div>
                  <p className="eyebrow">{ultimate.kind} · ultimate {ultimate.id}</p>
                  <h3>{ultimate.displayName}</h3>
                </div>
                <code>{ultimate.key}</code>
              </div>
              <NumericTreeEditor
                value={ultimate.parameters}
                path={["ultimates", index, "parameters"]}
                onNumber={controller.setNumber}
              />
            </article>
          ))}
        </div>
      </section>

      <section className="panel">
        <p className="eyebrow">Damageable environment · V10</p>
        <h2>Oil barrel</h2>
        <p>Authoritative health, blast damage, radius, target budget, and chain ceiling.</p>
        <NumericTreeEditor
          value={draft.barrel}
          path={["barrel"]}
          onNumber={controller.setNumber}
        />
      </section>

      <section className="panel">
        <p className="eyebrow">Contested restoration · V10</p>
        <h2>Treasure chest &amp; pickup</h2>
        <p>Chest health, restoration amount, collection radius, and lifetime for the next clean Practice epoch.</p>
        <NumericTreeEditor
          value={draft.chest}
          path={["chest"]}
          onNumber={controller.setNumber}
        />
      </section>

      <section className="panel">
        <p className="eyebrow">Mode objective · V10</p>
        <h2>Heist safe</h2>
        <p>Authoritative maximum health applied on the next clean Heist Practice epoch.</p>
        <NumericTreeEditor
          value={draft.heist}
          path={["heist"]}
          onNumber={controller.setNumber}
        />
      </section>

      <footer className="actionbar">
        <div>
          <strong>{controller.dirty ? "Unapplied draft" : "Draft matches the server"}</strong>
          <small>
            {controller.connected
              ? "Apply starts a clean authoritative Practice epoch."
              : "The next Practice worker will reload your last applied tuning."}
          </small>
        </div>
        <div className="actions">
          <button disabled={busy || !controller.dirty} onClick={controller.revert}>Revert draft</button>
          <button disabled={busy} onClick={() => void controller.restore()}>Restore defaults</button>
          <button className="primary" disabled={busy || !controller.dirty} onClick={() => void controller.apply()}>
            Apply &amp; reset
          </button>
        </div>
      </footer>
    </main>
  );
}
