import { createContext, useContext, useEffect, useState, type ReactNode } from "react"

import { isDemo, setDemoLanguage } from "../lib/api"

export type Language = "en" | "pt"

const STORAGE_KEY = "cortex.language"

type LanguageContextValue = {
  language: Language
  setLanguage: (language: Language) => void
  /** Whether a person may change it. The desktop app has one reader and he
   *  reads Portuguese; the demo is for whoever opens the link. */
  switchable: boolean
}

/**
 * Two defaults on purpose.
 *
 * The demo opens in English because it exists to be read by people who are not
 * the author. The installed app opens in Portuguese and never offers the
 * switch, because translating a window nobody else uses is work with no reader.
 */
const LanguageContext = createContext<LanguageContextValue>({
  language: "pt",
  setLanguage: () => {},
  switchable: false,
})

function chosen(): Language {
  if (!isDemo) return "pt"
  const saved = localStorage.getItem(STORAGE_KEY) as Language | null
  if (saved === "en" || saved === "pt") return saved
  // A Brazilian visitor gets Portuguese without hunting for the toggle;
  // everyone else gets English, which is who the demo is for.
  return navigator.language?.toLowerCase().startsWith("pt") ? "pt" : "en"
}

// Told before React renders, not in an effect: effects run after the first
// paint, and a search fired on that paint would read the wrong vault.
setDemoLanguage(chosen())

export function LanguageProvider({ children }: { children: ReactNode }) {
  const [language, setLanguage] = useState<Language>(chosen)

  useEffect(() => {
    setDemoLanguage(language)
    if (isDemo) localStorage.setItem(STORAGE_KEY, language)
    document.documentElement.lang = language === "pt" ? "pt-BR" : "en"
  }, [language])

  return (
    <LanguageContext.Provider value={{ language, setLanguage, switchable: isDemo }}>
      {children}
    </LanguageContext.Provider>
  )
}

export const useLanguage = () => useContext(LanguageContext)

/** Picks the side of a `{ en, pt }` pair for the language in force.
 *
 *  Kept as a hook so a component writes `pick(content)` once rather than
 *  threading `language` through every line it renders. */
export function usePick(): <T>(pair: { en: T; pt: T }) => T {
  const { language } = useLanguage()
  return (pair) => pair[language]
}
