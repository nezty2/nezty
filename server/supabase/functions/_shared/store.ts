// Opslag: de rijen zoals ze in de database staan, en wat de licentielogica van de opslag nodig heeft.
// Er zijn twee uitvoeringen: Supabase (echt) en een bestand in het geheugen (alleen om lokaal te testen).

export type LicenseType = "month" | "demo";

export interface LicenseRow {
  id: string;
  /** HMAC-SHA256 van de sleutel. De sleutel zelf wordt nooit opgeslagen. */
  key_hash: string;
  /** Laatste 4 tekens, om de sleutel te herkennen. */
  key_hint: string;
  type: LicenseType;
  duration_minutes: number;
  created_at: string;
  activated_at: string | null;
  expires_at: string | null;
  revoked_at: string | null;
  user_id: string | null;
  device_id: string | null;
  note: string | null;
}

export interface UserRow {
  id: string;
  username: string;
  username_lower: string;
  password_hash: string;
  email: string | null;
  created_at: string;
  last_login_at: string | null;
  failed_logins: number;
  locked_until: string | null;
}

export interface SessionRow {
  id: string;
  /** SHA-256 van het sessiegeheim. */
  token_hash: string;
  user_id: string | null;
  license_id: string;
  device_id: string;
  created_at: string;
  last_seen_at: string;
  expires_at: string;
}

export interface Store {
  licenseByHash(hash: string): Promise<LicenseRow | null>;
  licenseById(id: string): Promise<LicenseRow | null>;
  licenseByUser(userId: string): Promise<LicenseRow | null>;
  licensesByIdPrefix(prefix: string): Promise<LicenseRow[]>;
  listLicenses(): Promise<LicenseRow[]>;
  insertLicense(row: LicenseRow): Promise<void>;
  updateLicense(id: string, patch: Partial<LicenseRow>): Promise<void>;
  /** Koppelt een ongebruikte licentie aan een gebruiker. False als een ander net eerder was. */
  claimLicense(id: string, patch: Partial<LicenseRow>): Promise<boolean>;

  userByName(usernameLower: string): Promise<UserRow | null>;
  userById(id: string): Promise<UserRow | null>;
  listUsers(): Promise<UserRow[]>;
  /** False als de gebruikersnaam al bestaat. */
  insertUser(row: UserRow): Promise<boolean>;
  updateUser(id: string, patch: Partial<UserRow>): Promise<void>;

  sessionByHash(hash: string): Promise<SessionRow | null>;
  insertSession(row: SessionRow): Promise<void>;
  updateSession(id: string, patch: Partial<SessionRow>): Promise<void>;
  deleteSession(id: string): Promise<void>;
  deleteSessionsWhere(field: "user_id" | "license_id", value: string): Promise<void>;

  demoDeviceUsed(deviceId: string): Promise<boolean>;
  markDemoDevice(deviceId: string): Promise<void>;

  getSettings(): Promise<Record<string, unknown>>;
  setSetting(key: string, value: unknown): Promise<void>;
}
