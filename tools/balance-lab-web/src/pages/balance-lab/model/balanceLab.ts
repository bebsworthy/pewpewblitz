export type JsonObject = { [key: string]: JsonValue };
export type JsonValue = null | boolean | number | string | JsonObject | JsonValue[];
export type EditorPath = (string | number)[];

export type EditorSection =
  | "fighters"
  | "weapons"
  | "ultimates"
  | "world-objects"
  | "modes";

export interface EditorFieldDescriptor {
  path: EditorPath;
  section: EditorSection;
  subjectKey: string;
  subjectLabel: string;
  group: string;
  label: string;
  storageKind: "integer" | "decimal";
  unit: string;
  storageScale: number;
  minimum: number;
  maximum: number;
  minimumExclusive: boolean;
  step: number;
  control: "number" | "range-and-number";
  help?: string;
}

export interface BalanceLabEditorManifest {
  schemaVersion: number;
  fields: EditorFieldDescriptor[];
}

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

export interface ChestTuning extends JsonObject {
  damageProfile: JsonObject;
  pickupDefinition: JsonObject;
}

export interface BalanceLabSnapshot extends JsonObject {
  schemaVersion: number;
  fighterProfiles: FighterProfiles;
  weapons: WeaponTuning[];
  ultimates: UltimateTuning[];
  barrel: BarrelTuning;
  chest: ChestTuning;
  heist: HeistTuning;
}

export interface TransactionView {
  id: number;
  status: "pending" | "applied" | "rejected";
  message: string;
}

export interface LoadoutChoice {
  id: number;
  key: string;
  displayName: string;
}

export interface ScalarModifier {
  flat: number;
  percentBasisPoints: number;
}

export interface SlowModifier {
  penaltyBasisPoints: number;
  durationTicks: number;
}

export interface WeaponModifiers {
  capacity: ScalarModifier;
  damage: ScalarModifier;
  fireInterval: ScalarModifier;
  refillInterval: ScalarModifier;
  reachMilliunits: ScalarModifier;
  slow: SlowModifier | null;
}

export interface PlayerLoadout {
  playerId: string;
  displayName: string;
  team: number;
  participantType: "human" | "bot";
  fighterProfile: LoadoutChoice;
  weaponBase: LoadoutChoice;
  ultimate: LoadoutChoice;
  passives: [LoadoutChoice, LoadoutChoice];
  weaponModifiers: WeaponModifiers;
}

export interface BalanceLabState {
  schemaVersion: number;
  matchId: string;
  revision: number;
  players: PlayerLoadout[];
  editorManifest: BalanceLabEditorManifest;
  baseline: BalanceLabSnapshot;
  applied: BalanceLabSnapshot;
  pending: TransactionView | null;
  lastTransaction: TransactionView | null;
}
