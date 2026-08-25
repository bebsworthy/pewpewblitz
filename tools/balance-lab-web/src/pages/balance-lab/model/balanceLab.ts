export type JsonObject = { [key: string]: JsonValue };
export type JsonValue = null | boolean | number | string | JsonObject | JsonValue[];

export interface FighterStats extends JsonObject {
  maximum_health: number;
  movement_speed: number;
  reveal_proximity_radius: number;
}

export interface FighterProfiles extends JsonObject {
  default: FighterStats;
  lightweight: FighterStats;
  reinforced: FighterStats;
}

export interface WeaponTuning extends JsonObject {
  id: number;
  key: string;
  displayName: string;
  recipe: JsonObject;
}

export interface UltimateTuning extends JsonObject {
  id: number;
  key: string;
  displayName: string;
  kind: string;
  parameters: JsonObject;
}

export interface BarrelTuning extends JsonObject {
  damageProfile: JsonObject;
  explosionProfile: JsonObject;
}

export interface HeistTuning extends JsonObject {
  safeMaximumHealth: number;
}

export interface BalanceLabSnapshot extends JsonObject {
  schemaVersion: number;
  fighterProfiles: FighterProfiles;
  weapons: WeaponTuning[];
  ultimates: UltimateTuning[];
  barrel: BarrelTuning;
  heist: HeistTuning;
}

export interface TransactionView {
  id: number;
  status: "pending" | "applied" | "rejected";
  message: string;
}

export interface BalanceLabState {
  schemaVersion: number;
  matchId: string;
  revision: number;
  baseline: BalanceLabSnapshot;
  applied: BalanceLabSnapshot;
  pending: TransactionView | null;
  lastTransaction: TransactionView | null;
}
