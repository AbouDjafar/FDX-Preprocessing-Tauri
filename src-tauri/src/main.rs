#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod service;
use fdx_core::models::{error, JobControl, Progress};
use serde::Serialize;
use service::{AppState, Request};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{
    ipc::{Channel, JavaScriptChannelId},
    Manager, State,
};

#[derive(Serialize)]
struct ApiError {
    code: String,
    message: String,
}

#[tauri::command]
async fn fdx(
    request: Request,
    on_event: Option<JavaScriptChannelId>,
    webview: tauri::Webview,
    state: State<'_, Arc<AppState>>,
) -> std::result::Result<serde_json::Value, ApiError> {
    let app = state.inner().clone();
    let task_id = request.task_id.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut tasks = app.tasks.lock().map_err(|_| ApiError {
            code: "state".into(),
            message: "Etat indisponible. Relancez l'application.".into(),
        })?;
        if tasks.contains_key(&task_id) {
            return Err(ApiError {
                code: "duplicate".into(),
                message: "Cette operation est deja en cours.".into(),
            });
        }
        tasks.insert(task_id.clone(), cancel.clone());
    }
    let progress_task = task_id.clone();
    let job = JobControl {
        cancelled: cancel,
        progress: on_event.map(|id| {
            let channel: Channel<Progress> = id.channel_on(webview);
            Arc::new(move |mut event: Progress| {
                event.task_id = progress_task.clone();
                let _ = channel.send(event);
            }) as Arc<dyn Fn(Progress) + Send + Sync>
        }),
    };
    let run_state = app.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || run_state.execute(request, &job)).await;
    if let Ok(mut tasks) = app.tasks.lock() {
        tasks.remove(&task_id);
    }
    result
        .map_err(|e| error(format!("Le traitement s'est interrompu : {e}")))
        .and_then(|r| r)
        .map_err(|e| ApiError {
            code: if matches!(e, fdx_core::models::FdxError::Cancelled) {
                "cancelled"
            } else {
                "operation"
            }
            .into(),
            message: e.to_string(),
        })
}

#[tauri::command]
fn cancel_task(task_id: String, state: State<'_, Arc<AppState>>) -> bool {
    if let Ok(tasks) = state.tasks.lock() {
        if let Some(cancel) = tasks.get(&task_id) {
            cancel.store(true, Ordering::Relaxed);
            return true;
        }
    }
    false
}

fn startup_size(width: u32, height: u32, scale: f64) -> tauri::LogicalSize<f64> {
    tauri::LogicalSize::new(
        (f64::from(width) / scale - 32.0).clamp(400.0, 1280.0),
        (f64::from(height) / scale - 72.0).clamp(280.0, 850.0),
    )
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = app.path().app_local_data_dir()?;
            app.manage(Arc::new(AppState::new(root)?));
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = window.current_monitor() {
                    let area = monitor.work_area();
                    window.set_size(startup_size(
                        area.size.width,
                        area.size.height,
                        monitor.scale_factor(),
                    ))?;
                    window.center()?;
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![fdx, cancel_task])
        .run(tauri::generate_context!())
        .expect("Impossible de demarrer FDX Atelier");
}

#[cfg(test)]
mod tests {
    use super::startup_size;

    #[test]
    fn startup_fits_small_and_scaled_work_areas() {
        for (width, height) in [(1024, 720), (1366, 720), (1920, 1032)] {
            for scale in [1.0, 1.25, 1.5, 2.0] {
                let size = startup_size(width, height, scale);
                assert!(size.width + 16.0 <= f64::from(width) / scale);
                assert!(size.height + 40.0 <= f64::from(height) / scale);
                assert!(size.width <= 1280.0 && size.height <= 850.0);
            }
        }
    }
}
