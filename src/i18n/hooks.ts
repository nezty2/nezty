import { useCallback } from "react";
import type { Lang } from "@/lib/types";
import { useApp } from "@/store/app";
import { hasKey, translate, translateList, type Params, type TKey } from "./index";

export function useLang(): Lang {
  return useApp((s) => s.lang);
}

/** Hook: `t("nav.dashboard")` met controle op sleutels, `td(...)` voor samengestelde sleutels. */
export function useT() {
  const lang = useLang();
  const t = useCallback((key: TKey, params?: Params) => translate(lang, key, params), [lang]);
  const td = useCallback(
    (key: string, params?: Params, fallback?: string) => {
      if (fallback !== undefined && !hasKey(lang, key) && !hasKey("en", key)) return fallback;
      return translate(lang, key, params);
    },
    [lang],
  );
  const tl = useCallback((key: string) => translateList(lang, key), [lang]);
  return { t, td, tl, lang };
}
