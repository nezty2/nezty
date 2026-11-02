import { Gamepad2, Play, Plus, RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useT } from "@/i18n/hooks";
import { api, pickExe } from "@/lib/ipc";
import { useApp } from "@/store/app";
import { GameCover } from "./GameCover";
import { NowTab } from "./GameTabs";
import { Loader } from "./Loader";
import { Badge, Button, Card, EmptyState, SearchInput, cn } from "./ui";

/** Per game: kies een game en zet tweaks voor die game direct aan of uit. */
export function PerGame() {
  const { t } = useT();
  const games = useApp((s) => s.games);
  const loading = useApp((s) => s.gamesLoading);
  const refreshGames = useApp((s) => s.refreshGames);
  const runningGames = useApp((s) => s.runningGames);
  const handleError = useApp((s) => s.handleError);
  const toast = useApp((s) => s.toast);
  const pageParams = useApp((s) => s.pageParams);

  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<string | null>(pageParams.game ?? null);
  const [adding, setAdding] = useState(false);
  const detailRef = useRef<HTMLDivElement>(null);
  const reduceMotion = useApp((s) => s.settings?.reduceMotion ?? false);

  // Vanaf het dashboard: meteen de aangeklikte game openen.
  useEffect(() => {
    if (pageParams.game) setSelected(pageParams.game);
  }, [pageParams.game]);

  const filtered = useMemo(() => games.filter((g) => g.name.toLowerCase().includes(search.toLowerCase())), [games, search]);
  const game = games.find((g) => g.id === selected) ?? null;
  const running = !!game && runningGames.includes(game.id);

  // Na het kiezen van een game rustig naar de instellingen van die game scrollen.
  useEffect(() => {
    if (!game) return;
    const id = window.setTimeout(() => detailRef.current?.scrollIntoView({ behavior: reduceMotion ? "auto" : "smooth", block: "start" }), 60);
    return () => window.clearTimeout(id);
    // Alleen bij een andere game, niet bij elke wijziging in de lijst.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game?.id]);

  const addGame = async () => {
    const path = await pickExe(t("games.add"));
    if (!path) return;
    setAdding(true);
    try {
      const g = await api.addCustomGame(path, null);
      toast({ level: "success", title: t("notice.success"), message: t("games.added", { name: g.name }) });
      await refreshGames();
      setSelected(g.id);
    } catch (e) {
      handleError(e);
    } finally {
      setAdding(false);
    }
  };

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-[13px] text-ink-2">{t("games.subtitle")}</p>
        <div className="flex flex-wrap items-center gap-2">
          <SearchInput value={search} onChange={setSearch} placeholder={t("games.search")} className="w-48" />
          <Button size="sm" icon={RefreshCw} onClick={() => void refreshGames()} loading={loading}>
            {t("games.rescan")}
          </Button>
          <Button size="sm" variant="primary" icon={Plus} onClick={() => void addGame()} loading={adding}>
            {t("games.add")}
          </Button>
        </div>
      </div>

      {loading && games.length === 0 ? (
        <Loader />
      ) : filtered.length === 0 ? (
        <EmptyState icon={Gamepad2} title={games.length === 0 ? t("games.emptyTitle") : t("games.noMatch")} description={games.length === 0 ? t("games.emptyDesc") : undefined} />
      ) : (
        <div className="grid grid-cols-3 gap-3 md:grid-cols-4 2xl:grid-cols-6">
          {filtered.map((g) => {
            const isRunning = runningGames.includes(g.id);
            const active = selected === g.id;
            return (
              <button
                key={g.id}
                onClick={() => setSelected(active ? null : g.id)}
                className={cn("card group overflow-hidden text-left transition-colors", active ? "border-accent ring-1 ring-accent/40" : "hover:border-line-2")}
              >
                <div className="relative aspect-[460/215] overflow-hidden border-b border-line">
                  <GameCover game={g} className="transition-transform duration-300 group-hover:scale-[1.03]" />
                  <div className="absolute left-1.5 top-1.5 flex gap-1">
                    {isRunning && <Badge variant="ok" className="!bg-[#0f1115]/90">{t("games.playing")}</Badge>}
                  </div>
                </div>
                <div className="px-2.5 py-2">
                  <div className="truncate text-[13px] font-semibold">{g.name}</div>
                  <div className="truncate text-[11px] text-ink-3">
                    {t(`launcher.${g.launcher}`)}
                    {!g.exePath && <span> · {t("games.noExe")}</span>}
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      )}

      {game && (
        <div ref={detailRef} className="scroll-mt-24">
        <Card>
          <div className="flex flex-wrap items-center gap-3">
            <span className="h-10 w-[86px] shrink-0 overflow-hidden rounded-md border border-line">
              <GameCover game={game} />
            </span>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2 text-sm font-semibold">
                {game.name}
                {running && <Badge variant="ok">{t("games.playing")}</Badge>}
              </div>
              <div className="text-[12px] text-ink-3">{t(`launcher.${game.launcher}`)}</div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              {game.launcher === "custom" && (
                <Button
                  size="sm"
                  variant="danger"
                  icon={Trash2}
                  onClick={async () => {
                    try {
                      await api.removeCustomGame(game.id);
                      toast({ level: "info", title: t("notice.info"), message: t("games.removed", { name: game.name }) });
                      setSelected(null);
                      void refreshGames();
                    } catch (e) {
                      handleError(e);
                    }
                  }}
                >
                  {t("common.remove")}
                </Button>
              )}
              <Button size="sm" icon={Play} onClick={() => api.launchGame(game.id).catch(handleError)}>
                {t("games.launch")}
              </Button>
            </div>
          </div>

          <div className="mt-5">
            <NowTab game={game} />
          </div>
        </Card>
        </div>
      )}
    </div>
  );
}
