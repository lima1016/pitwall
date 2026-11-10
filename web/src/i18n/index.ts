import { create } from 'zustand'
import { en, type MessageKey } from './en.ts'
import { ko } from './ko.ts'

export type Lang = 'ko' | 'en'
export const LANGS: readonly Lang[] = ['ko', 'en']

const dictionaries: Record<Lang, Record<MessageKey, string>> = { ko, en }
const STORAGE_KEY = 'pitwall.lang'

export function detectLang(): Lang {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved === 'ko' || saved === 'en') return saved
  } catch {
    // localStorage를 못 쓰는 환경이면 브라우저 언어로
  }
  return navigator.language.toLowerCase().startsWith('ko') ? 'ko' : 'en'
}

interface LangState {
  lang: Lang
  setLang: (lang: Lang) => void
}

export const useLang = create<LangState>((set) => ({
  lang: detectLang(),
  setLang: (lang) => {
    try {
      localStorage.setItem(STORAGE_KEY, lang)
    } catch {
      // 저장 실패해도 이번 화면에서는 바뀐다
    }
    document.documentElement.lang = lang
    set({ lang })
  },
}))

export function translate(lang: Lang, key: MessageKey, vars?: Record<string, string>): string {
  let text = dictionaries[lang][key] ?? en[key]
  for (const [k, v] of Object.entries(vars ?? {})) text = text.replaceAll(`{${k}}`, v)
  return text
}

export function useT() {
  const lang = useLang((s) => s.lang)
  return (key: MessageKey, vars?: Record<string, string>) => translate(lang, key, vars)
}

export type { MessageKey }
