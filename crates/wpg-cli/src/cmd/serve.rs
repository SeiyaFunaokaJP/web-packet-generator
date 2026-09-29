//! `wpg serve`: load the configuration, set up logging, print the banner and run
//! the HTTP server until Ctrl-C / SIGTERM.

use std::net::SocketAddr;
use std::path::Path;
use std::process::ExitCode;

use wpg_i18n::{t, Locale};
use wpg_server::config::DEFAULT_CONFIG_FILE;
use wpg_server::store::ListFilter;
use wpg_server::{AppState, Config, Overrides, ServeOptions, Server, DEV_ORIGINS, VERSION};

use crate::cmd::cli_locale;
use crate::cmd::send::tr;

/// Where the configuration came from, for the banner.
pub(crate) fn config_source(explicit: Option<&Path>, locale: Locale) -> String {
    match explicit {
        Some(p) => p.display().to_string(),
        None if Path::new(DEFAULT_CONFIG_FILE).exists() => DEFAULT_CONFIG_FILE.to_owned(),
        None => t(locale, "config-source-defaults"),
    }
}

/// The localised startup banner lines.
pub(crate) fn banner(
    locale: Locale,
    config: &Config,
    addr: SocketAddr,
    source: &str,
    state: &AppState,
    dev: bool,
) -> Vec<String> {
    let addr_s = addr.to_string();
    let mut out = vec![
        tr(
            locale,
            "serve-banner-title",
            &[
                ("app", t(locale, "app-name")),
                ("version", VERSION.to_owned()),
            ],
        ),
        tr(locale, "serve-banner-listen", &[("addr", addr_s.clone())]),
        tr(locale, "serve-banner-docs", &[("addr", addr_s)]),
        tr(
            locale,
            "serve-banner-config",
            &[("source", source.to_owned())],
        ),
        tr(
            locale,
            "serve-banner-data",
            &[("path", config.data_dir.display().to_string())],
        ),
    ];
    for lib in &config.templates.library_paths {
        let state_text = if lib.is_dir() {
            String::new()
        } else {
            t(locale, "serve-banner-missing")
        };
        out.push(tr(
            locale,
            "serve-banner-library",
            &[("path", lib.display().to_string()), ("state", state_text)],
        ));
    }
    out.push(tr(
        locale,
        "serve-banner-user-dir",
        &[("path", config.user_template_dir().display().to_string())],
    ));
    out.push(tr(
        locale,
        "serve-banner-templates",
        &[(
            "count",
            state
                .templates
                .list(&ListFilter::default())
                .len()
                .to_string(),
        )],
    ));
    let default = config.default_backend();
    let list: Vec<String> = wpg_tx::backend::available()
        .into_iter()
        .map(|b| {
            let mark = if b.available { '✓' } else { '✗' };
            let star = if b.name == default { "*" } else { "" };
            format!("{mark} {}{star}", b.name)
        })
        .collect();
    out.push(tr(
        locale,
        "serve-banner-backends",
        &[("list", list.join("  "))],
    ));
    if dev {
        out.push(tr(
            locale,
            "serve-banner-dev",
            &[("origins", DEV_ORIGINS.join(", "))],
        ));
    }
    out
}

/// Runs `wpg serve`.
pub(crate) fn run(args: &crate::ServeArgs) -> ExitCode {
    let locale = cli_locale();
    let overrides = Overrides {
        bind: args.bind,
        library_paths: args.library_paths.clone(),
        data_dir: args.data_dir.clone(),
    };
    let config = match Config::load(args.config.as_deref(), &overrides) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "{}",
                tr(locale, "config-load-error", &[("error", e.to_string())])
            );
            return ExitCode::from(2);
        }
    };
    let _log_guard = match wpg_server::init_tracing(&config.log) {
        Ok(g) => g,
        Err(e) => {
            eprintln!(
                "{}",
                tr(locale, "serve-log-error", &[("error", e.to_string())])
            );
            return ExitCode::FAILURE;
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!(
                "{}",
                tr(locale, "serve-failed", &[("error", e.to_string())])
            );
            return ExitCode::FAILURE;
        }
    };
    let source = config_source(args.config.as_deref(), locale);
    let dev = args.dev;
    let result = runtime.block_on(async move {
        let server = Server::bind(config.clone(), ServeOptions { dev, watch: true }).await?;
        let addr = server.local_addr()?;
        for line in banner(locale, &config, addr, &source, server.state(), dev) {
            eprintln!("{line}");
        }
        tracing::info!("listening on {addr}");
        if !config.has_password() {
            tracing::warn!(
                "{}",
                tr(
                    locale,
                    "serve-banner-no-password",
                    &[("addr", addr.to_string())]
                )
            );
        }
        server.run().await
    });
    // Stop background threads (file watcher, blocking pool) promptly.
    runtime.shutdown_timeout(std::time::Duration::from_secs(2));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!(
                "{}",
                tr(locale, "serve-failed", &[("error", e.to_string())])
            );
            ExitCode::FAILURE
        }
    }
}
