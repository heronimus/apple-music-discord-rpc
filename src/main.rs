mod error;
mod handlers;
mod models;
mod observer;
mod utils;

use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use objc2::rc::autoreleasepool;
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSPort, NSRunLoop};
use objc2_media_player::MPMusicPlayerController;

fn main() -> Result<(), Box<dyn Error>> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        println!("Shutting down client...");
        r.store(false, Ordering::SeqCst);
    })?;

    unsafe {
        println!("DEBUG: Registering Observer");
        let dummy_player = MPMusicPlayerController::systemMusicPlayer();
        let _observer = observer::MusicPlayerObserver::new();

        let run_loop = NSRunLoop::currentRunLoop();

        // Add a port to the run loop to keep it active
        let port = NSPort::port(); // or create a new port
        run_loop.addPort_forMode(&port, NSDefaultRunLoopMode);

        while running.load(Ordering::SeqCst) {
            autoreleasepool(|_| {
                // Use a shorter interval to be more responsive
                let date = NSDate::dateWithTimeIntervalSinceNow(5.00);

                // Use runMode with a shorter timeout
                run_loop.runMode_beforeDate(NSDefaultRunLoopMode, &date);

                _ = dummy_player.indexOfNowPlayingItem();
            });

            // Optional: Small sleep to prevent CPU spinning
            // std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    Ok(())
}
