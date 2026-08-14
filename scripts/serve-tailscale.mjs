#!/usr/bin/env node
/**
 * Puts real HTTPS in front of the Cortex server, so the phone can open the
 * vault from anywhere on the tailnet.
 *
 * The server itself is inside the app — this only routes tailnet traffic to it,
 * and takes the mapping back down on the way out.
 *
 * Port 8443 rather than 443, to leave any mapping already on 443 alone.
 *
 *   npm run serve:remote
 */
import { execFileSync } from "node:child_process";
import { connect } from "node:net";

const APP_PORT = Number(process.env.CORTEX_PORT ?? 8787);
const HTTPS_PORT = Number(process.env.HTTPS_PORT ?? 8443);

const run = (args, options = {}) =>
  execFileSync("tailscale", args, { encoding: "utf8", ...options });

function fail(message) {
  console.error(`\n✖ ${message}\n`);
  process.exit(1);
}

/** This machine's MagicDNS name, which also proves the daemon is reachable. */
function tailscaleHost() {
  let status;
  try {
    status = JSON.parse(run(["status", "--json"], { stdio: ["ignore", "pipe", "ignore"] }));
  } catch {
    fail(
      "Não consegui falar com o Tailscale. Instale e conecte:\n" +
        "  brew install tailscale && tailscale up",
    );
  }
  if (status.BackendState !== "Running") {
    fail(`O Tailscale não está conectado (estado: ${status.BackendState}). Rode: tailscale up`);
  }
  const name = status.Self?.DNSName?.replace(/\.$/, "");
  if (!name) fail("O Tailscale não informou um nome MagicDNS para esta máquina.");
  return name;
}

/** No point routing traffic to a port with nothing behind it. */
function assertAppIsUp(port) {
  return new Promise((resolve) => {
    const probe = connect({ port, host: "127.0.0.1" })
      .once("error", () =>
        fail(
          `Nada escutando na porta ${port}. O servidor sobe junto com o Cortex —\n` +
            "abra o app e rode isto de novo.",
        ),
      )
      .once("connect", () => probe.end(resolve));
  });
}

const host = tailscaleHost();
await assertAppIsUp(APP_PORT);

try {
  run(["serve", "--bg", `--https=${HTTPS_PORT}`, `http://127.0.0.1:${APP_PORT}`], {
    stdio: ["ignore", "ignore", "inherit"],
  });
} catch {
  fail(
    `Não consegui subir o "tailscale serve" na porta ${HTTPS_PORT}.\n` +
      "Se os certificados HTTPS estiverem desligados no tailnet, ligue no painel de admin.",
  );
}

console.log(`\n  ➜  Tailnet:  https://${host}:${HTTPS_PORT}\n`);
console.log("  O endereço com o código de sessão está no botão de compartilhar do app.\n");
console.log("  Para desligar:  tailscale serve --https=" + HTTPS_PORT + " off\n");
