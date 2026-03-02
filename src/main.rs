use lean_graph::MApp;
mod __file_nat_zero;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "lean graph",
        native_options,
        Box::new(|cc| Box::new(MApp::new(cc, __file_nat_zero::DATA.into()))),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    // Redirect `log` message to `console.log` and friends:

    use lean_graph::read_graph_url;
    use lean_graph::SERVER_ADDR;

    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let mut web_options = eframe::WebOptions::default();
    if has_dark_hash() {
        web_options.follow_system_theme = false;
        web_options.default_theme = eframe::Theme::Dark;
    }

    // Check for a `url` query parameter to load custom extracted data JSON
    let json_url = get_url_query_param("url")
        .unwrap_or_else(|| format!("{}/static/Nat.zero_add.json", SERVER_ADDR));
    let default_json_url = format!("{}/static/Nat.zero_add.json", SERVER_ADDR);

    wasm_bindgen_futures::spawn_local(async move {
        // Try custom/default URL first, then fall back to embedded JSON to keep app bootable.
        let data_raw = match read_graph_url(&json_url).await {
            Ok(data) => data,
            Err(err) => {
                log::warn!("Failed to load JSON from '{}': {}. Falling back.", json_url, err);
                match read_graph_url(&default_json_url).await {
                    Ok(default_data) => default_data,
                    Err(default_err) => {
                        log::warn!(
                            "Failed to load default JSON from '{}': {}. Using embedded DATA.",
                            default_json_url,
                            default_err
                        );
                        __file_nat_zero::DATA.to_owned()
                    }
                }
            }
        };

        eframe::WebRunner::new()
            .start(
                "lean-graph-canvas", // hardcode it
                web_options,
                Box::new(|cc| Box::new(MApp::new(cc, data_raw))),
            )
            .await
            .expect("failed to start eframe");
    });
}

#[cfg(target_arch = "wasm32")]
fn get_url_query_param(param: &str) -> Option<String> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    params.get(param)
}

#[cfg(target_arch = "wasm32")]
fn has_dark_hash() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(hash) = window.location().hash() else {
        return false;
    };
    hash.eq_ignore_ascii_case("#dark")
}
