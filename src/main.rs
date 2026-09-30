#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_os = "windows"))]
compile_error!("monitor2 requires Windows");

mod hardware;
mod nvml;
mod settings;
mod settings_ui;
mod topology;
mod ui;

use hardware::{Sampler, Snapshot};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [] => run(false, false),
        [arg] if arg == "--smoke-test" => run(true, false),
        [arg] if arg == "--probe" => probe(),
        [arg] if arg == "--preview" => run(false, true),
        _ => Err("Usage: monitor2 [--probe | --smoke-test | --preview]".into()),
    };
    if let Err(error) = result {
        eprintln!("monitor2: {error}");
        if args.is_empty() {
            ui::show_error(&error);
        }
        std::process::exit(1);
    }
}

fn probe() -> Result<(), String> {
    let mut sampler = Sampler::new();
    for _ in 0..3 {
        thread::sleep(Duration::from_secs(1));
        let sample = sampler.sample();
        println!("{sample:#?}");
        println!(
            "Age: {:?}, CPU error: {:?}",
            sample.sampled_at.map(|t| t.elapsed()),
            sample.cpu_error
        );
        for gpu in &sample.gpus {
            println!("GPU error: {:?}", gpu.error);
        }
        if let Some(ram) = sample.ram {
            println!("RAM usage: {:.1}%", ram.percent());
        }
    }
    Ok(())
}

fn run(smoke_test: bool, preview: bool) -> Result<(), String> {
    let latest = Arc::new(Mutex::new(Snapshot::default()));
    let settings = Arc::new(settings::SharedSettings::new(settings::Settings::load()?));
    let worker_latest = Arc::clone(&latest);
    let worker_settings = Arc::clone(&settings);
    let worker = thread::Builder::new()
        .name("hardware-sampler".into())
        .spawn(move || {
            // Construct and drop every PDH/NVML handle on the sampling thread.
            let mut sampler = Sampler::new();
            loop {
                let sample = sampler.sample();
                *worker_latest.lock().unwrap_or_else(|e| e.into_inner()) = sample;
                if worker_settings.wait() {
                    break;
                }
            }
        })
        .map_err(|e| format!("Cannot start sampler: {e}"))?;
    let result = ui::run(latest, settings.clone(), smoke_test, preview);
    settings.stop();
    worker
        .join()
        .map_err(|_| "Hardware sampler panicked".to_string())?;
    result
}
