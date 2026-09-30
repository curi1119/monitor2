#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_os = "windows"))]
compile_error!("monitor2 requires Windows");

mod hardware;
mod nvml;
mod ui;

use hardware::{Sampler, Snapshot};
use std::{
    sync::{Arc, Condvar, Mutex},
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
        println!("{:#?}", sampler.sample());
    }
    Ok(())
}

fn run(smoke_test: bool, preview: bool) -> Result<(), String> {
    let latest = Arc::new(Mutex::new(Snapshot::default()));
    let stop = Arc::new((Mutex::new(false), Condvar::new()));
    let worker_latest = Arc::clone(&latest);
    let worker_stop = Arc::clone(&stop);
    let worker = thread::Builder::new()
        .name("hardware-sampler".into())
        .spawn(move || {
            // Construct and drop every PDH/NVML handle on the sampling thread.
            let mut sampler = Sampler::new();
            loop {
                let sample = sampler.sample();
                *worker_latest.lock().unwrap_or_else(|e| e.into_inner()) = sample;
                let (lock, wake) = &*worker_stop;
                let stopped = lock.lock().unwrap_or_else(|e| e.into_inner());
                let (stopped, _) = wake
                    .wait_timeout_while(stopped, Duration::from_secs(1), |v| !*v)
                    .unwrap_or_else(|e| e.into_inner());
                if *stopped {
                    break;
                }
            }
        })
        .map_err(|e| format!("Cannot start sampler: {e}"))?;
    let result = ui::run(latest, smoke_test, preview);
    let (lock, wake) = &*stop;
    *lock.lock().unwrap_or_else(|e| e.into_inner()) = true;
    wake.notify_one();
    worker
        .join()
        .map_err(|_| "Hardware sampler panicked".to_string())?;
    result
}
