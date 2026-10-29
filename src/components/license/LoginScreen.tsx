import { motion } from "framer-motion";
import { AlertCircle, KeyRound, LogIn, Play, UserPlus, WifiOff, Zap } from "lucide-react";
import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { Button, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { api, asBackendError } from "@/lib/ipc";
import { formatKeyInput, keyChars } from "@/lib/license";
import { useApp } from "@/store/app";
import { ContactInfo } from "./LicenseParts";

type Tab = "register" | "login";
type Panel = null | "forgot" | "request";

function Field({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1.5 flex items-baseline justify-between gap-2 text-[13px] font-medium text-ink-2">
        {label}
        {hint && <span className="text-[12px] font-normal text-ink-3">{hint}</span>}
      </span>
      {children}
    </label>
  );
}

function Input(props: { value: string; onChange: (v: string) => void; type?: string; autoComplete?: string; placeholder?: string; mono?: boolean; autoFocus?: boolean; invalid?: boolean }) {
  return (
    <input
      value={props.value}
      onChange={(e) => props.onChange(e.target.value)}
      type={props.type ?? "text"}
      autoComplete={props.autoComplete}
      placeholder={props.placeholder}
      autoFocus={props.autoFocus}
      spellCheck={false}
      className={cn(
        "h-10 w-full rounded-lg border bg-panel-2 px-3 text-sm text-ink placeholder:text-ink-3 focus:outline-none",
        props.invalid ? "border-bad/60 focus:border-bad" : "border-line focus:border-accent",
        props.mono && "font-mono tracking-wide",
      )}
    />
  );
}

/** Inloggen of account maken, direct na de openingsanimatie. */
export function LoginScreen() {
  const { t, td } = useT();
  const license = useApp((s) => s.license);
  const config = useApp((s) => s.licenseConfig);
  const loadConfig = useApp((s) => s.loadLicenseConfig);
  const setLicense = useApp((s) => s.setLicense);
  const [tab, setTab] = useState<Tab>("login");
  const [panel, setPanel] = useState<Panel>(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [key, setKey] = useState("");
  const [email, setEmail] = useState("");
  const [remember, setRemember] = useState(true);
  const [busy, setBusy] = useState<null | "submit" | "demo">(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  // Uitgelogd door de server (bv. apparaat losgekoppeld of wachtwoord gereset): laten zien waarom.
  const reason = license?.reason ? td(`msg.lic_${license.reason}`, undefined, "") : "";
  const keyOk = keyChars(key) !== null;
  const keyTyped = key.replace(/[^0-9A-Z]/gi, "").length >= 19;

  const show = (e: unknown) => {
    const err = asBackendError(e);
    setError(err.key ? td(`msg.${err.key}`, err.params, t("msg.lic_server_error")) : t("msg.lic_server_error"));
  };

  const submit = async (ev: FormEvent) => {
    ev.preventDefault();
    setError(null);
    if (tab === "register" && !keyOk) {
      setError(t("msg.lic_key_format"));
      return;
    }
    setBusy("submit");
    try {
      const st = tab === "login" ? await api.licenseLogin(username, password, remember) : await api.licenseRegister(username, password, key, email || null, remember);
      setLicense(st);
    } catch (e) {
      show(e);
    } finally {
      setBusy(null);
    }
  };

  const demo = async () => {
    setError(null);
    setBusy("demo");
    try {
      setLicense(await api.licenseDemo());
    } catch (e) {
      show(e);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="fixed inset-0 z-[90] flex items-center justify-center overflow-y-auto px-4 py-10">
      <motion.div initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }} className="w-full max-w-[420px]">
        <div className="mb-6 flex items-center justify-center gap-3">
          <div className="btn-primary flex h-10 w-10 items-center justify-center rounded-xl">
            <Zap className="h-5 w-5 text-white" fill="currentColor" />
          </div>
          <div className="text-xl font-semibold tracking-tight">nezty</div>
        </div>

        <div className="card p-6">
          <div className="mb-5 grid grid-cols-2 gap-1 rounded-lg border border-line bg-panel-2 p-1">
            {(["login", "register"] as Tab[]).map((id) => (
              <button
                key={id}
                type="button"
                onClick={() => {
                  setTab(id);
                  setError(null);
                  setPanel(null);
                }}
                className={cn("flex h-9 items-center justify-center gap-2 rounded-md text-sm font-medium transition-colors", tab === id ? "bg-accent-soft text-ink ring-1 ring-accent/30" : "text-ink-2 hover:text-ink")}
              >
                {id === "login" ? <LogIn className="h-4 w-4" /> : <UserPlus className="h-4 w-4" />}
                {t(`login.tab.${id}`)}
              </button>
            ))}
          </div>

          {reason && !error && (
            <p className="mb-4 flex items-start gap-2 rounded-lg border border-warn/40 bg-warn/10 px-3 py-2 text-[13px] text-ink">
              <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-warn" />
              {reason}
            </p>
          )}

          <form onSubmit={(e) => void submit(e)} className="space-y-4">
            <Field label={t("login.username")}>
              <Input value={username} onChange={setUsername} autoComplete="username" autoFocus />
            </Field>
            <Field label={t("login.password")} hint={tab === "register" ? t("login.passwordHint") : undefined}>
              <Input value={password} onChange={setPassword} type="password" autoComplete={tab === "login" ? "current-password" : "new-password"} />
            </Field>
            {tab === "register" && (
              <>
                <Field label={t("login.key")} hint={keyTyped ? (keyOk ? <span className="text-ok">{t("login.keyLooksOk")}</span> : <span className="text-bad">{t("login.keyTypo")}</span>) : undefined}>
                  <Input value={key} onChange={(v) => setKey(formatKeyInput(v))} placeholder="NZT-XXXX-XXXX-XXXX-XXXX" mono invalid={keyTyped && !keyOk} />
                </Field>
                <Field label={t("login.email")} hint={t("login.optional")}>
                  <Input value={email} onChange={setEmail} type="email" autoComplete="email" />
                </Field>
              </>
            )}

            <label className="flex cursor-pointer items-center gap-2.5 text-[13px] text-ink-2">
              <input type="checkbox" checked={remember} onChange={(e) => setRemember(e.target.checked)} className="h-4 w-4 accent-[var(--accent)]" />
              {t("login.remember")}
            </label>

            {error && (
              <p className="flex items-start gap-2 rounded-lg border border-bad/40 bg-bad/10 px-3 py-2 text-[13px] text-ink">
                <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-bad" />
                {error}
              </p>
            )}

            <Button type="submit" variant="primary" className="h-10 w-full justify-center" loading={busy === "submit"} disabled={!username || !password || busy !== null} icon={tab === "login" ? LogIn : KeyRound}>
              {tab === "login" ? t("login.submitLogin") : t("login.submitRegister")}
            </Button>
          </form>

          {config?.demoEnabled && (
            <div className="mt-4 border-t border-line pt-4">
              <Button variant="secondary" className="h-10 w-full justify-center" icon={Play} loading={busy === "demo"} disabled={busy !== null} onClick={() => void demo()}>
                {t("login.demo")}
              </Button>
              <p className="mt-1.5 text-center text-[12px] text-ink-3">{t("login.demoNote")}</p>
            </div>
          )}

          {config?.reachable === false && (
            <p className="mt-4 flex items-start gap-2 text-[12px] text-warn">
              <WifiOff className="mt-0.5 h-3.5 w-3.5 shrink-0" />
              {t("login.offline")}
            </p>
          )}
        </div>

        <div className="mt-4 flex items-center justify-center gap-4 text-[13px]">
          <button type="button" onClick={() => setPanel(panel === "forgot" ? null : "forgot")} className="text-ink-3 hover:text-ink">
            {t("login.forgot")}
          </button>
          <span className="text-line-2">·</span>
          <button type="button" onClick={() => setPanel(panel === "request" ? null : "request")} className="text-ink-3 hover:text-ink">
            {t("login.request")}
          </button>
        </div>

        {panel && (
          <motion.div initial={{ opacity: 0, y: -4 }} animate={{ opacity: 1, y: 0 }} className="card mt-3 space-y-3 p-5">
            <p className="text-[13px] leading-relaxed text-ink-2">{panel === "forgot" ? t("login.forgotText") : t("login.requestText")}</p>
            <ContactInfo config={config} compact />
          </motion.div>
        )}
      </motion.div>
    </div>
  );
}
