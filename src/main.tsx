import React from "react"
import ReactDOM from "react-dom/client"

// Fonts ship with the app: a desktop tool cannot depend on a CDN being reachable.
import "@fontsource-variable/archivo"
import "@fontsource-variable/jetbrains-mono"

import App from "./App"
import { ThemeProvider } from "./components/theme-provider"
import "./index.css"

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      <App />
    </ThemeProvider>
  </React.StrictMode>,
)
