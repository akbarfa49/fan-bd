use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use fan_bd::core;
use fan_bd::engine;
use image::math::Rect;
use scap::capturer;
use scap::targets;
// use fan_bd::engine;
// use std::process::Stdio;
// use tokio::io::{AsyncBufReadExt, BufReader};
// use core::result;
use crossterm::{
    ExecutableCommand, QueueableCommand, cursor,
    style::Print,
    terminal::{self, ClearType},
};
use std::io::stdout;
use tokio::process::Command;
use tokio::spawn;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize core
    let core = core::Core::new()?;

    // Start the capture loop in background
    core.start().await?;

    // Get a receiver for loot updates
    let mut stdout = stdout();
    // Main loop - process updates as they come
    loop {
        let loot_updates = core.get_current_loot().await;
        // println!("{:?}", loot_updates);
        stdout.execute(cursor::MoveTo(0, 0)).unwrap();
        stdout.execute(terminal::Clear(ClearType::All)).unwrap();
        for (_, v) in loot_updates {
            println!("{}: {}", v.name, v.amount);
        }
        tokio::select! {
                // This branch executes when new loot data arrives


                // Add other events here as needed
                _ = tokio::signal::ctrl_c() => {
                    println!("Ctrl+C received, shutting down");
                    break;
                }

                // Add a small sleep to prevent busy-waiting
                _ = tokio::time::sleep(Duration::from_millis(5000)) => {

                }
        // Move cursor to top and clear


        }
    }
    Ok(())
}
