import { useCallback, useEffect, useState } from "react"

/**
 * A preference this device keeps to itself.
 *
 * The Mac and the phone are the same app talking to the same vault, so anything
 * stored in the vault would be a setting for both. Whether the AI joins a search
 * is a choice about the screen in front of you — and keeping it here means the
 * phone can flip it without a write route on the server.
 */
export function useLocalToggle(key: string, fallback = false) {
  const [on, setOn] = useState(() => {
    try {
      const stored = localStorage.getItem(key)
      return stored === null ? fallback : stored === "1"
    } catch {
      // Private browsing refuses storage; the app still has to open.
      return fallback
    }
  })

  useEffect(() => {
    try {
      localStorage.setItem(key, on ? "1" : "0")
    } catch {
      /* nothing to do about it, and nothing worth breaking over */
    }
  }, [key, on])

  return [on, useCallback(() => setOn((value) => !value), [])] as const
}