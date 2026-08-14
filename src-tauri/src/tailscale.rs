//! Talking to Tailscale from inside the app, so the phone never needs a terminal.
//!
//! Everything here shells out to the `tailscale` CLI. Finding it is half the
//! work: an app opened from the Finder inherits no `PATH` worth the name, which
//! is why the address panel worked in development and would have shown nothing
//! at all in the installed app.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use serde_json::Value;

/// The port `tailscale serve` answers on. Deliberately not 443: a mapping may
/// already live there for something else, and taking it would break that.
pub const HTTPS_PORT: u16 = 8443;

/// Where the CLI turns up, most specific first. The `PATH` comes first so an app
/// opened from a terminal uses whatever that terminal has; the rest are the
/// places the two macOS installers put it.
fn candidates() -> Vec<PathBuf> {
    let mut found = vec![PathBuf::from("tailscale")];
    found.push(PathBuf::from("/opt/homebrew/bin/tailscale"));
    found.push(PathBuf::from("/usr/local/bin/tailscale"));
    found.push(PathBuf::from("/Applications/Tailscale.app/Contents/MacOS/Tailscale"));
    if let Some(home) = dirs::home_dir() {
        found.push(home.join("Applications/Tailscale.app/Contents/MacOS/Tailscale"));
    }
    found
}

/// The first candidate that answers. Looked up each time rather than cached to
/// disk: reinstalling Tailscale somewhere else must not leave the app pointing
/// at a path that no longer exists.
pub fn binary() -> Option<PathBuf> {
    candidates().into_iter().find(|path| {
        Command::new(path)
            .arg("version")
            .output()
            .is_ok_and(|output| output.status.success())
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// False means the CLI was not found anywhere; the panel then explains how
    /// to install it instead of hiding the whole tab.
    pub installed: bool,
    pub connected: bool,
    /// What the daemon called its own state, shown as-is when it is not running.
    pub state: String,
    /// MagicDNS name of this machine, when there is one.
    pub host: Option<String>,
    /// Whether `tailscale serve` is already pointing at our port.
    pub serving: bool,
}

pub fn status(app_port: u16) -> Status {
    let Some(binary) = binary() else {
        return Status {
            installed: false,
            connected: false,
            state: String::new(),
            host: None,
            serving: false,
        };
    };

    let json: Option<Value> = Command::new(&binary)
        .args(["status", "--json"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| serde_json::from_slice(&output.stdout).ok());

    let state = json
        .as_ref()
        .and_then(|value| value.get("BackendState"))
        .and_then(Value::as_str)
        .unwrap_or("Desconhecido")
        .to_string();

    let host = json
        .as_ref()
        .and_then(|value| value.get("Self"))
        .and_then(|value| value.get("DNSName"))
        .and_then(Value::as_str)
        .map(|name| name.trim_end_matches('.').to_string())
        .filter(|name| !name.is_empty());

    Status {
        installed: true,
        connected: state == "Running",
        serving: is_serving(&binary, app_port),
        state,
        host,
    }
}

/// Whether our own mapping is up. Read from Tailscale rather than remembered,
/// so the switch shows what is true even after the app was killed.
fn is_serving(binary: &Path, app_port: u16) -> bool {
    Command::new(binary)
        .args(["serve", "status"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
        .is_some_and(|text| mentions_our_mapping(&text, app_port))
}

/// The listing groups a port with the targets under it, so both have to appear
/// for the mapping to be ours — another project's mapping on another port must
/// never read as ours.
fn mentions_our_mapping(listing: &str, app_port: u16) -> bool {
    let target = format!("http://127.0.0.1:{app_port}");
    let mut ours = false;

    for line in listing.lines() {
        if line.starts_with("https://") {
            ours = line.contains(&format!(":{HTTPS_PORT}"));
        } else if ours && line.contains(&target) {
            return true;
        }
    }
    false
}

/// Points the tailnet at the app. Only ever touches `HTTPS_PORT` — a mapping on
/// any other port belongs to something else.
pub fn start(app_port: u16) -> Result<(), String> {
    let binary = binary().ok_or_else(|| "Tailscale não encontrado nesta máquina".to_string())?;
    let output = Command::new(binary)
        .args([
            "serve",
            "--bg",
            &format!("--https={HTTPS_PORT}"),
            &format!("http://127.0.0.1:{app_port}"),
        ])
        .output()
        .map_err(|error| error.to_string())?;

    if output.status.success() {
        return Ok(());
    }
    let complaint = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if complaint.is_empty() {
        "o Tailscale recusou o mapeamento".into()
    } else {
        complaint
    })
}

pub fn stop() -> Result<(), String> {
    let Some(binary) = binary() else { return Ok(()) };
    Command::new(binary)
        .args(["serve", &format!("--https={HTTPS_PORT}"), "off"])
        .output()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub label: String,
    pub ok: bool,
    /// What happened, in words. Empty when there is nothing to add.
    pub detail: String,
}

/// Walks the chain the phone walks, and names the link that broke.
///
/// The last step is a real request over the tailnet, which is the only way to
/// catch HTTPS being disabled for the tailnet — every earlier step passes in
/// that case. It goes through `/usr/bin/curl`, always present on macOS: linking
/// a TLS stack into the app for one button is not a trade worth making.
pub fn test(app_port: u16) -> Vec<Step> {
    let mut steps = Vec::new();
    let status = status(app_port);

    steps.push(Step {
        label: "Tailscale instalado".into(),
        ok: status.installed,
        detail: match binary() {
            Some(path) => path.display().to_string(),
            None => "comando não encontrado em nenhum caminho conhecido".into(),
        },
    });
    if !status.installed {
        return steps;
    }

    steps.push(Step {
        label: "conectado ao tailnet".into(),
        ok: status.connected,
        detail: if status.connected {
            status.host.clone().unwrap_or_default()
        } else {
            format!("o daemon respondeu {}", status.state)
        },
    });
    let Some(host) = status.host.clone().filter(|_| status.connected) else {
        return steps;
    };

    steps.push(Step {
        label: format!("mapeamento na porta {HTTPS_PORT}"),
        ok: status.serving,
        detail: if status.serving {
            format!("aponta para 127.0.0.1:{app_port}")
        } else {
            "ligue o interruptor do Tailscale".into()
        },
    });
    if !status.serving {
        return steps;
    }

    steps.push(reach(&format!("https://{host}:{HTTPS_PORT}/health")));
    steps
}

/// The same last step for the local address, where the failure is usually the
/// server being off rather than anything to do with the network.
pub fn reach_local(address: &str, app_port: u16) -> Step {
    reach(&format!("http://{address}:{app_port}/health"))
}

fn reach(url: &str) -> Step {
    // No session here on purpose: /health is the one route without a cookie, so
    // the token never rides on a command line where `ps` would show it.
    let output = Command::new("/usr/bin/curl")
        .args(["--silent", "--show-error", "--max-time", "8", url])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let body = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Step {
                label: "o endereço responde".into(),
                ok: body == HEALTH_BODY,
                detail: if body == HEALTH_BODY {
                    url.to_string()
                } else {
                    format!("respondeu outra coisa: {}", body.chars().take(60).collect::<String>())
                },
            }
        }
        Ok(output) => Step {
            label: "o endereço responde".into(),
            ok: false,
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        },
        Err(error) => Step {
            label: "o endereço responde".into(),
            ok: false,
            detail: error.to_string(),
        },
    }
}

/// What `/health` answers. Shared so the check compares against one constant.
pub const HEALTH_BODY: &str = "cortex";

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = "\
https://macbook.tail0000.ts.net (tailnet only)
|-- / proxy http://127.0.0.1:3000

https://macbook.tail0000.ts.net:8443 (tailnet only)
|-- / proxy http://127.0.0.1:8787
";

    #[test]
    fn our_mapping_is_recognised_by_port_and_target_together() {
        assert!(mentions_our_mapping(LISTING, 8787));
    }

    /// The mapping on 443 belongs to another project on this machine. Reading it
    /// as ours would mean the switch offering to turn it off.
    #[test]
    fn another_projects_mapping_is_not_ours() {
        assert!(!mentions_our_mapping(LISTING, 3000));
    }

    #[test]
    fn the_right_port_with_the_wrong_target_is_not_ours() {
        let listing = "https://macbook.tail0000.ts.net:8443 (tailnet only)\n|-- / proxy http://127.0.0.1:9999\n";
        assert!(!mentions_our_mapping(listing, 8787));
    }

    #[test]
    fn an_empty_listing_means_nothing_is_served() {
        assert!(!mentions_our_mapping("", 8787));
    }

    /// The lookup has to start with the bare name, or an app opened from a
    /// terminal would ignore the Tailscale that terminal can see.
    #[test]
    fn the_search_starts_with_the_path_and_covers_both_installers() {
        let paths = candidates();
        assert_eq!(paths[0], PathBuf::from("tailscale"));
        assert!(paths.iter().any(|p| p.ends_with("opt/homebrew/bin/tailscale")));
        assert!(paths.iter().any(|p| p.to_string_lossy().contains("Tailscale.app")));
    }
}