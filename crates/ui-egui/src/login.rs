//! Startup login for a build that has an ERP environment file. The document stays behind this
//! until a saved token is still good, a refresh replaces it, or the person signs in.

use serde_json::{Value, json};

use crate::i18n::tr;
use crate::widgets;
use crate::{FigApp, theme};

/// Draw the login and skip the rest of the window when this build requires one and nobody is
/// signed in. A saved token that has run out is refreshed here, without asking again.
pub(crate) fn gate(app: &mut FigApp, ui: &mut egui::Ui) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if !crate::erp::configured() {
            return false;
        }
        if ensure(app) {
            return false;
        }
        draw(app, ui);
        true
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (app, ui);
        false
    }
}

/// The access token is the one to send. Refreshes it when it has run out and a refresh token
/// is still kept.
pub(crate) fn ensure(app: &mut FigApp) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        restore(app);
        if crate::erp::access_current(&app.session.auth_token, app.session.auth_expires) {
            return true;
        }
        if app.auth_hold {
            return false;
        }
        let refresh = app.session.auth_refresh.clone();
        if refresh.is_empty() {
            return false;
        }
        match crate::erp::refresh(&refresh) {
            Ok(signed) => {
                apply(app, signed);
                true
            }
            Err(e) => {
                app.auth_hold = true;
                let expired = e == crate::erp::EXPIRED;
                app.ui.live.login_error = e;
                if expired {
                    forget(app);
                    app.ui.live.login_error = crate::erp::EXPIRED.to_string();
                }
                false
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        !app.session.auth_token.is_empty()
    }
}

/// Drop the saved login. The next frame asks again.
pub(crate) fn forget(app: &mut FigApp) {
    app.run("auth.signOut", json!({}));
    app.ui.live.login_pass.clear();
    app.services.save_auth("");
}

#[cfg(not(target_arch = "wasm32"))]
fn restore(app: &mut FigApp) {
    if app.auth_restored {
        return;
    }
    app.auth_restored = true;
    let Some(body) = app.services.load_auth() else { return };
    let Ok(v) = serde_json::from_str::<Value>(&body) else { return };
    let Some(access) = v.get("access").and_then(Value::as_str).filter(|s| !s.is_empty()) else { return };
    app.run(
        "auth.signIn",
        json!({
            "token": access,
            "refresh": v.get("refresh").and_then(Value::as_str).unwrap_or(""),
            "name": v.get("name").and_then(Value::as_str).unwrap_or(""),
            "expires": v.get("expires").and_then(Value::as_i64).unwrap_or(0),
            "refreshExpires": v.get("refreshExpires").and_then(Value::as_i64).unwrap_or(0),
        }),
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn apply(app: &mut FigApp, signed: crate::erp::SignedIn) {
    app.auth_hold = false;
    app.ui.live.login_error.clear();
    app.run(
        "auth.signIn",
        json!({
            "token": signed.access,
            "refresh": signed.refresh,
            "name": signed.name,
            "expires": signed.expires,
            "refreshExpires": signed.refresh_expires,
        }),
    );
    remember(app);
}

#[cfg(not(target_arch = "wasm32"))]
fn remember(app: &mut FigApp) {
    if app.session.auth_token.is_empty() {
        return;
    }
    let body = json!({
        "access": app.session.auth_token,
        "refresh": app.session.auth_refresh,
        "name": app.session.auth_name,
        "expires": app.session.auth_expires,
        "refreshExpires": app.session.auth_refresh_expires,
    });
    app.services.save_auth(&body.to_string());
}

#[cfg(not(target_arch = "wasm32"))]
fn draw(app: &mut FigApp, ui: &mut egui::Ui) {
    let t = theme::Tokens::get(ui.ctx());
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.canvas)).show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(120.0);
            widgets::label(ui, tr("Sign in to use FigCraft."), t.text);
            ui.add_space(12.0);
            if let Some(name) = widgets::text_field_named(ui, egui::Id::unique("startup_login_user"), &app.ui.live.login_user, 220.0, tr("Username")) {
                app.ui.live.login_user = name.chars().filter(|c| !c.is_control()).take(80).collect();
            }
            ui.add_space(8.0);
            if let Some(pass) = widgets::secret_field_named(ui, egui::Id::unique("startup_login_pass"), &app.ui.live.login_pass, 220.0, tr("Password")) {
                app.ui.live.login_pass = pass.chars().filter(|c| !c.is_control()).take(200).collect();
            }
            ui.add_space(12.0);
            if widgets::button(ui, tr("Log in"), true, true).clicked() {
                let user = app.ui.live.login_user.clone();
                let pass = std::mem::take(&mut app.ui.live.login_pass);
                match crate::erp::login(&user, &pass) {
                    Ok(signed) => apply(app, signed),
                    Err(e) => app.ui.live.login_error = e,
                }
            }
            if !app.ui.live.login_error.is_empty() {
                ui.add_space(8.0);
                widgets::label(ui, &app.ui.live.login_error, t.danger);
            }
        });
    });
}
