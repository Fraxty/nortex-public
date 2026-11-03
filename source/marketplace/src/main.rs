use nortex_marketplace::{router, AppState, Config};

#[tokio::main]
async fn main() {
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("configuration error: {e}");
            std::process::exit(2);
        }
    };
    let bind = cfg.bind.clone();
    let state = match AppState::new(cfg) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("startup error: {e}");
            std::process::exit(2);
        }
    };

    // `create-admin <username>` reads the password from NORTEX_MARKET_ADMIN_PASSWORD.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("create-admin") {
        let (Some(user), Ok(pw)) = (args.get(2), std::env::var("NORTEX_MARKET_ADMIN_PASSWORD"))
        else {
            eprintln!("usage: NORTEX_MARKET_ADMIN_PASSWORD=... nortex-marketplace create-admin <username>");
            std::process::exit(2);
        };
        match state.create_user(user, &pw, "admin") {
            Ok(()) => println!("admin '{user}' created"),
            Err(e) => {
                eprintln!("failed: {}", e.message());
                std::process::exit(1);
            }
        }
        return;
    }

    if !state.signing_configured() {
        eprintln!(
            "warning: NORTEX_MARKET_SIGNING_KEY_HEX not set; publishing and downloads return 503"
        );
    }
    let listener = match tokio::net::TcpListener::bind(&bind).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot bind {bind}: {e}");
            std::process::exit(1);
        }
    };
    println!("nortex-marketplace listening on http://{bind} (plain HTTP; terminate TLS in a reverse proxy)");
    let _ = axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await;
}
