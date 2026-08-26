import { useState } from "react";
import { weaponModifierLabels } from "../lib/playerLoadouts";
import type { PlayerLoadout } from "../model/balanceLab";

export function PlayerLoadouts({ players }: { players: PlayerLoadout[] }) {
  const [expanded, setExpanded] = useState(true);
  return (
    <section className="player-loadouts" aria-labelledby="player-loadouts-heading">
      <header>
        <div>
          <p className="eyebrow">Authoritative roster</p>
          <h2 id="player-loadouts-heading">Players &amp; loadouts</h2>
          <p>Admitted builds used by this Practice worker.</p>
        </div>
        <div className="loadout-heading-actions">
          <span>{players.length} participants</span>
          <button
            type="button"
            aria-expanded={expanded}
            aria-controls="player-loadout-grid"
            onClick={() => setExpanded((current) => !current)}
          >
            {expanded ? "Hide loadouts" : "Show loadouts"}
          </button>
        </div>
      </header>
      {expanded && <div className="loadout-grid" id="player-loadout-grid">
        {players.map((player) => {
          const modifiers = weaponModifierLabels(player.weaponModifiers);
          return (
            <article className={`loadout-card team-${player.team}`} key={player.playerId}>
              <header>
                <div>
                  <h3>{player.displayName}</h3>
                </div>
                <div className="loadout-badges">
                  <span>Team {player.team + 1}</span>
                  <span>{player.participantType}</span>
                </div>
              </header>
              <dl>
                <div><dt>Fighter</dt><dd>{player.fighterProfile.displayName}</dd></div>
                <div><dt>Weapon</dt><dd>{player.weaponBase.displayName}</dd></div>
                <div><dt>Ultimate</dt><dd>{player.ultimate.displayName}</dd></div>
                <div>
                  <dt>Passives</dt>
                  <dd>{player.passives.map((passive) => passive.displayName).join(" · ")}</dd>
                </div>
              </dl>
              <div className="modifier-list">
                <strong>Effective weapon modifiers</strong>
                {modifiers.length > 0 ? (
                  <ul>{modifiers.map((modifier) => <li key={modifier}>{modifier}</li>)}</ul>
                ) : (
                  <small>None</small>
                )}
              </div>
            </article>
          );
        })}
      </div>}
    </section>
  );
}
