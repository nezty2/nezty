-- nezty licenties. Plak dit in de SQL Editor van Supabase (of gebruik `supabase db push`).
-- Alle tabellen hebben Row Level Security aan en géén policies: alleen de serverfunctie
-- (met de service role) kan erbij. De app en de buitenwereld kunnen de tabellen niet lezen.

create table if not exists public.users (
  id uuid primary key,
  username text not null,
  username_lower text not null unique,
  password_hash text not null,          -- pbkdf2$rondes$zout$hash, nooit het wachtwoord zelf
  email text,
  created_at timestamptz not null default now(),
  last_login_at timestamptz,
  failed_logins integer not null default 0,
  locked_until timestamptz
);

create table if not exists public.licenses (
  id uuid primary key,
  key_hash text not null unique,        -- HMAC van de sleutel, nooit de sleutel zelf
  key_hint text not null,               -- laatste 4 tekens
  type text not null check (type in ('month', 'demo')),
  duration_minutes integer not null,
  created_at timestamptz not null default now(),
  activated_at timestamptz,
  expires_at timestamptz,
  revoked_at timestamptz,
  user_id uuid references public.users(id) on delete set null,
  device_id text,
  note text
);
create unique index if not exists licenses_user_once on public.licenses(user_id) where user_id is not null;

create table if not exists public.sessions (
  id uuid primary key,
  token_hash text not null unique,      -- SHA-256 van het sessiegeheim
  user_id uuid references public.users(id) on delete cascade,
  license_id uuid not null references public.licenses(id) on delete cascade,
  device_id text not null,
  created_at timestamptz not null default now(),
  last_seen_at timestamptz not null default now(),
  expires_at timestamptz not null
);
create index if not exists sessions_user on public.sessions(user_id);
create index if not exists sessions_license on public.sessions(license_id);

create table if not exists public.demo_devices (
  device_id text primary key,
  used_at timestamptz not null default now()
);

create table if not exists public.settings (
  key text primary key,
  value jsonb not null
);
insert into public.settings (key, value) values
  ('demo_enabled', 'false'::jsonb),
  ('contact', '{"discord": "", "email": "", "website": "", "note": ""}'::jsonb)
on conflict (key) do nothing;

alter table public.users enable row level security;
alter table public.licenses enable row level security;
alter table public.sessions enable row level security;
alter table public.demo_devices enable row level security;
alter table public.settings enable row level security;
