use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

use crate::AppState;

pub struct TrayHandles {
    pub tray: TrayIcon,
    pub zapret_item: MenuItem<tauri::Wry>,
    pub tg_item: MenuItem<tauri::Wry>,
    pub warp_item: MenuItem<tauri::Wry>,
}

struct TrayLabels {
    open: &'static str,
    zapret_on: &'static str,
    zapret_off: &'static str,
    tg_on: &'static str,
    tg_off: &'static str,
    warp_on: &'static str,
    warp_off: &'static str,
    quit: &'static str,
}

fn labels(lang: &str) -> TrayLabels {
    if lang == "en" {
        TrayLabels {
            open: "Open EasyZapret",
            zapret_on: "Zapret: turn off",
            zapret_off: "Zapret: turn on",
            tg_on: "Telegram Proxy: turn off",
            tg_off: "Telegram Proxy: turn on",
            warp_on: "Cloudflare WARP: turn off",
            warp_off: "Cloudflare WARP: turn on",
            quit: "Quit",
        }
    } else {
        TrayLabels {
            open: "Открыть EasyZapret",
            zapret_on: "Zapret: выключить",
            zapret_off: "Zapret: включить",
            tg_on: "Telegram Proxy: выключить",
            tg_off: "Telegram Proxy: включить",
            warp_on: "Cloudflare WARP: выключить",
            warp_off: "Cloudflare WARP: включить",
            quit: "Выход",
        }
    }
}

/// Renders a simple round status dot as the tray icon, so the icon can
/// reflect state without shipping binary image assets.
/// gray = all off, teal = something is on, red = error.
fn make_icon(color: (u8, u8, u8)) -> Image<'static> {
    const SIZE: usize = 32;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];
    let center = (SIZE as f32 - 1.0) / 2.0;
    let radius = SIZE as f32 * 0.42;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let alpha = if dist <= radius - 1.0 {
                255.0
            } else if dist <= radius + 1.0 {
                // soft edge
                (1.0 - (dist - (radius - 1.0)) / 2.0) * 255.0
            } else {
                0.0
            };
            let idx = (y * SIZE + x) * 4;
            rgba[idx] = color.0;
            rgba[idx + 1] = color.1;
            rgba[idx + 2] = color.2;
            rgba[idx + 3] = alpha.clamp(0.0, 255.0) as u8;
        }
    }
    Image::new_owned(rgba, SIZE as u32, SIZE as u32)
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let lang = crate::settings::load().language.unwrap_or_else(|| "ru".into());
    let l = labels(&lang);

    let open_item = MenuItem::with_id(app, "open", l.open, true, None::<&str>)?;
    let zapret_item = MenuItem::with_id(app, "toggle_zapret", l.zapret_off, true, None::<&str>)?;
    let tg_item = MenuItem::with_id(app, "toggle_tg", l.tg_off, true, None::<&str>)?;
    let warp_item = MenuItem::with_id(app, "toggle_warp", l.warp_off, true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", l.quit, true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[&open_item, &sep, &zapret_item, &tg_item, &warp_item, &sep2, &quit_item],
    )?;

    let tray = TrayIconBuilder::with_id("main-tray")
        .icon(make_icon((120, 128, 140)))
        .tooltip("EasyZapret")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "quit" => {
                // Full exit: stop processes we manage; an installed service
                // is intentionally left running (it is autonomous).
                let state = app.state::<AppState>();
                crate::vpn::disconnect_quiet();
                crate::warp::disconnect_quiet();
                crate::zapret::process::stop(&state);
                crate::tg_proxy::kill_all();
                app.exit(0);
            }
            "toggle_zapret" => {
                let state = app.state::<AppState>();
                if crate::zapret::process::winws_running() {
                    crate::zapret::process::stop(&state);
                } else {
                    let strategy = crate::settings::load()
                        .selected_strategy
                        .or_else(|| {
                            crate::zapret::strategy::list_strategies()
                                .ok()
                                .and_then(|v| v.first().cloned())
                        });
                    if let Some(bat) = strategy {
                        let _ = crate::zapret::process::start_strategy(&state, &bat);
                    }
                }
                update_tray_now(app);
            }
            "toggle_tg" => {
                if crate::tg_proxy::proxy_running() {
                    let _ = crate::tg_proxy::stop_tg();
                } else {
                    let _ = crate::tg_proxy::start_tg();
                }
                update_tray_now(app);
            }
            "toggle_warp" => {
                let state = app.state::<AppState>();
                if crate::warp::quick_status().connected {
                    let _ = crate::warp::warp_disconnect(state);
                } else {
                    // Silently ignored if Zapret is not running (WARP requires it).
                    let _ = crate::warp::warp_connect(state);
                }
                update_tray_now(app);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, .. } = event {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    let state = app.state::<AppState>();
    *state.tray.lock().unwrap() = Some(TrayHandles { tray, zapret_item, tg_item, warp_item });
    update_tray_now(app);
    Ok(())
}

/// Refreshes tray icon color, tooltip and toggle labels from current state.
pub fn update_tray_now(app: &AppHandle) {
    let zapret_on = crate::zapret::process::winws_running()
        || crate::zapret::service::query_service_state("zapret").as_deref() == Some("RUNNING");
    let tg_on = crate::tg_proxy::proxy_running();
    let warp_on = crate::warp::quick_status().connected;

    let lang = crate::settings::load().language.unwrap_or_else(|| "ru".into());
    let l = labels(&lang);

    let state = app.state::<AppState>();
    let guard = state.tray.lock().unwrap();
    if let Some(handles) = guard.as_ref() {
        let color = if zapret_on || tg_on || warp_on {
            (20, 184, 166) // teal — active
        } else {
            (120, 128, 140) // gray — idle
        };
        let _ = handles.tray.set_icon(Some(make_icon(color)));

        let on = |v: bool| if v { "ON" } else { "OFF" };
        let tooltip = format!(
            "EasyZapret — Zapret: {}, TG Proxy: {}, WARP: {}",
            on(zapret_on),
            on(tg_on),
            on(warp_on),
        );
        let _ = handles.tray.set_tooltip(Some(&tooltip));
        let _ = handles.zapret_item.set_text(if zapret_on { l.zapret_on } else { l.zapret_off });
        let _ = handles.tg_item.set_text(if tg_on { l.tg_on } else { l.tg_off });
        let _ = handles.warp_item.set_text(if warp_on { l.warp_on } else { l.warp_off });
    }
}
