use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use fan_bd::core;

// use fan_bd::engine;
// use std::process::Stdio;
// use tokio::io::{AsyncBufReadExt, BufReader};
// use core::result;
use crossterm::{
    ExecutableCommand, QueueableCommand, cursor,
    style::Print,
    terminal::{self, ClearType},
};
use fan_bd::engine::{ScreenConfig, Silver};
use std::io::stdout;
use tokio::process::Command;
use tokio::spawn;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize core
    // let pure_core: Result<core::Core<Arc<Mutex<scap::capturer::Capturer>>, core::Error> = core::Core::new();
    let mut core = core::Core::new().unwrap();
    core.use_drop().await;
    let game_screen = core.game_screen;
    core.use_capturer(core::config(0, 0, game_screen.width, game_screen.height, 1.0).unwrap());
    // Start the capture loop in background
    core.start().await;

    // Get a receiver for loot updates
    let mut stdout = stdout();
    // Main loop - process updates as they come
    loop {
        let loot_updates = core.get_current_loot().await;
        // println!("{:?}", loot_updates);
        stdout.execute(cursor::MoveTo(0, 0)).unwrap();
        stdout.execute(terminal::Clear(ClearType::All)).unwrap();
        let mut total_silver = Silver::new(0);
        for (_, v) in loot_updates {
            let silver = v.calculate();
            println!("({}){}: {}. {}", v.id, v.name, v.amount, silver);
            total_silver += silver;
        }
        println!("total silver: {}", total_silver);
        tokio::select! {
                // This branch executes when new loot data arrives


                // Add other events here as needed
                _ = tokio::signal::ctrl_c() => {
                    println!("Ctrl+C received, shutting down");
                    break;
                }

                // Add a small sleep to prevent busy-waiting
                _ = tokio::time::sleep(Duration::from_millis(10000)) => {

                }
        // Move cursor to top and clear


        }
    }
    Ok(())
}
