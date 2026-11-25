//! LifeOS Computer: a menu bar app that lets lifeos's agents see and work this Mac's screen (desktop use). It dials
//! the lifeos server over the link and runs the desktop tools the agents call, one at a time, through macOS's
//! accessibility, screenshots and input. Install it on as many Macs as you like; each shows up in Integrations.
//!
//!   lifeos-computer                    the menu bar app (what the .app runs)
//!   lifeos-computer --server <url>     sets the server, then runs
//!   lifeos-computer --check            prints what macOS grants and who this computer is, then exits

mod link;
#[cfg(target_os = "macos")]
mod menu;
mod shared;

use std::sync::Arc;

use anyhow::Result;

fn main() -> Result<()> {
    let shared = Arc::new(shared::Shared::load()?);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--server" => {
                let url = args.get(i + 1).ok_or_else(|| anyhow::anyhow!("--server takes a URL"))?;
                shared.set_server(url)?;
                i += 1;
            }
            "--check" => {
                println!("{:#?}", link::hello(&shared));
                println!("server: {}", shared.config().server);
                return Ok(());
            }
            // macOS passes this when the app is opened from Finder on old systems.
            a if a.starts_with("-psn_") => {}
            other => anyhow::bail!("unknown argument {other:?}; see --check, --server <url>"),
        }
        i += 1;
    }
    run(shared)
}

#[cfg(target_os = "macos")]
fn run(shared: Arc<shared::Shared>) -> Result<()> {
    // The desktop runs on a thread of its own, one tool at a time.
    let (jobs, inbox) = std::sync::mpsc::channel::<link::Job>();
    std::thread::Builder::new().name("desktop".into()).spawn(move || {
        let mut desktop = lifeos_desktop::Desktop::default();
        for job in inbox {
            let answer = desktop.run(&job.op, &job.input).map_err(|e| format!("{e:#}"));
            let _ = job.reply.send(answer);
        }
    })?;
    let linked = shared.clone();
    std::thread::Builder::new().name("link".into()).spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("tokio");
        rt.block_on(link::keep(linked, jobs));
    })?;
    // macOS asks the owner for what is missing, once.
    let _ = lifeos_desktop::Permissions::check(true);
    menu::run(shared);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn run(_shared: Arc<shared::Shared>) -> Result<()> {
    anyhow::bail!("LifeOS Computer runs on macOS")
}
