/**
 * Provider failures, said in this app's voice.
 *
 * A provider's own error text is written for whoever holds the billing account:
 * it names plans, links to rate-limit docs and tells you to check your quota.
 * Pasted into this window it reads like the app broke and is now asking you to
 * go pay someone.
 *
 * Only the failures that are actually known get translated. Anything else is
 * shown as it arrived — a message nobody can read is still better than one
 * nobody can debug.
 */

/** The vocabulary `provider::worth_falling_over` already sorts on, reused so
 *  the two layers agree about what "temporary" means. */
const RUNNING_OUT = ["rate limit", "rate_limit", "429", "quota", "exhausted", "resource_exhausted"]
const UNREACHABLE = ["timed out", "timeout", "connection", "resolve host", "unavailable", "dns"]
const NO_KEY = ["nenhuma chave", "no key", "api key", "unauthorized", "401", "403"]
/** A provider retiring a model is not an outage and not your key — it is a name
 *  in the settings that stopped existing, and only you can pick the new one. */
const GONE_MODEL = ["does not exist", "model_not_found", "decommissioned", "não existe"]

export function readable(error: string): string {
  const said = error.toLowerCase()

  if (said.includes("sem espaço") || said.includes("quotaexceeded")) {
    return "O navegador ficou sem espaço. Apagar um print ou voltar o vault ao original libera o que foi guardado aqui."
  }

  if (RUNNING_OUT.some((mark) => said.includes(mark))) {
    return "A cota do provedor acabou. O Groq costuma voltar em minutos; a do Gemini zera à meia-noite do Pacífico."
  }
  if (GONE_MODEL.some((mark) => said.includes(mark))) {
    return "Esse modelo saiu do ar no provedor. Troque o nome nas configurações — a lista atual está na documentação dele."
  }
  if (NO_KEY.some((mark) => said.includes(mark))) {
    return "Nenhuma chave respondeu. As configurações têm o campo, e o botão testar diz se ela está certa."
  }
  if (UNREACHABLE.some((mark) => said.includes(mark))) {
    return "O provedor não respondeu. Costuma ser a rede — perguntar de novo resolve."
  }
  return error
}
