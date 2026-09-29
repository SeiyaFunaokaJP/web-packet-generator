//! `wpg serve`: load the configuration, set up logging, print the banner and run
//! the HTTP server until Ctrl-C / SIGTERM.

use std::path::Path;
use std::process::ExitCode;

use wpg_i18n::{t, Locale};
use wpg_server::config::Secret;
use wpg_server::config::DEFAULT_CONFIG_FILE;
use wpg_server::store::ListFilter;
use wpg_server::{
    AppState, BindError, Config, Overrides, ServeOptions, Server, DEV_ORIGINS, VERSION,
};

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
    server: &Server,
    source: &str,
    dev: bool,
) -> Vec<String> {
    let state: &AppState = server.state();
    let addr = server.local_addr().unwrap_or(config.server.bind);
    let scheme = if server.tls().is_some() {
        "https"
    } else {
        "http"
    };
    let url = format!("{scheme}://{addr}");
    let mut out = vec![
        tr(
            locale,
            "serve-banner-title",
            &[
                ("app", t(locale, "app-name")),
                ("version", VERSION.to_owned()),
            ],
        ),
        tr(locale, "serve-banner-listen", &[("url", url.clone())]),
        tr(locale, "serve-banner-docs", &[("url", url)]),
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
    if let Some(t) = server.tls() {
        out.push(tr(
            locale,
            if t.self_signed {
                "serve-banner-tls-self-signed"
            } else {
                "serve-banner-tls"
            },
            &[("path", t.cert_path.display().to_string())],
        ));
        out.push(tr(
            locale,
            "serve-banner-fingerprint",
            &[("fingerprint", t.fingerprint.clone())],
        ));
    }
    let auth = if state.auth.setup_required() {
        t(locale, "serve-banner-auth-setup")
    } else {
        match state.auth.external_source() {
            Some(src) => tr(
                locale,
                "serve-banner-auth-configured",
                &[("source", src.to_owned())],
            ),
            None => t(locale, "serve-banner-auth-database"),
        }
    };
    out.push(tr(locale, "serve-banner-auth", &[("state", auth)]));
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
    if args.password.is_some() {
        eprintln!("{}", t(locale, "serve-password-arg-warning"));
    }
    let overrides = Overrides {
        bind: args.bind,
        library_paths: args.library_paths.clone(),
        data_dir: args.data_dir.clone(),
        password: args.password.clone().map(Secret::new),
        password_file: args.password_file.clone(),
        tls: args.tls,
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
    let opts = ServeOptions {
        dev: args.dev,
        watch: true,
        allow_insecure: args.allow_insecure,
    };
    let result = runtime.block_on(async move {
        let server = match Server::bind(config.clone(), opts).await {
            Ok(s) => s,
            Err(BindError::Insecure(addr)) => {
                let msg = tr(
                    locale,
                    "serve-refuse-insecure",
                    &[("addr", addr.to_string())],
                );
                tracing::error!("{msg}");
                return Ok(Some(ExitCode::from(2)));
            }
            Err(e) => return Err(std::io::Error::other(e)),
        };
        let addr = server.local_addr()?;
        for line in banner(locale, &config, &server, &source, opts.dev) {
            eprintln!("{line}");
        }
        tracing::info!("listening on {addr}");
        if server.state().auth.setup_required() {
            let scheme = if server.tls().is_some() {
                "https"
            } else {
                "http"
            };
            let key = if addr.ip().is_loopback() {
                "serve-banner-setup-local"
            } else {
                "serve-banner-no-password"
            };
            tracing::warn!(
                "{}",
                tr(locale, key, &[("url", format!("{scheme}://{addr}/"))])
            );
        }
        server.run().await.map(|()| None)
    });
    // Stop background threads (file watcher, blocking pool) promptly.
    runtime.shutdown_timeout(std::time::Duration::from_secs(2));
    match result {
        Ok(Some(code)) => code,
        Ok(None) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!(
                "{}",
                tr(locale, "serve-failed", &[("error", e.to_string())])
            );
            ExitCode::FAILURE
        }
    }
}
