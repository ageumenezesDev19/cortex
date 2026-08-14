import { useEffect, useState } from "react"

/**
 * How much screen there actually is, which on a phone is not what CSS says.
 *
 * `100dvh` on iOS is the height with the keyboard **closed**, and it does not
 * shrink when the keyboard opens. So the layout kept its full height, the bottom
 * of the app went under the keyboard, and Safari scrolled the whole page to
 * reveal the focused field — which is how the search field left the screen while
 * it was being typed into.
 *
 * `visualViewport` is the only thing that reports the truth here.
 */

/** Below this much lost height, it is the URL bar moving, not a keyboard. */
const KEYBOARD_MIN = 120

export function useViewportHeight() {
  const [keyboardOpen, setKeyboardOpen] = useState(false)

  useEffect(() => {
    const viewport = window.visualViewport
    const root = document.documentElement

    const measure = () => {
      const height = viewport?.height ?? window.innerHeight
      root.style.setProperty("--app-h", `${Math.round(height)}px`)

      // The layout viewport is what the keyboard does not shrink, so the gap
      // between the two is the keyboard.
      const hidden = window.innerHeight - height
      const open = hidden > KEYBOARD_MIN
      root.dataset.keyboard = open ? "open" : "closed"
      setKeyboardOpen(open)
    }

    measure()
    if (!viewport) return

    viewport.addEventListener("resize", measure)
    // Safari can still slide the layout viewport under the keyboard; this fires
    // when it does, and the shell is pinned again on the next frame.
    viewport.addEventListener("scroll", measure)
    return () => {
      viewport.removeEventListener("resize", measure)
      viewport.removeEventListener("scroll", measure)
    }
  }, [])

  return keyboardOpen
}
