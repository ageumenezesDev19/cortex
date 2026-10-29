//! The vault for coding agents. For now only the measurement; the MCP server
//! joins this binary, so hooks, evals and the server write the vault one way.

use std::process::ExitCode;

const USAGE: &str = "uso: cortex-mcp eval-rag [--label nome] [--gold caminho]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("eval-rag") => cortex_lib::eval::cli(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(report) => {
            println!("relatório: {}", report.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("erro: {error:#}");
            ExitCode::FAILURE
        }
    }
}
