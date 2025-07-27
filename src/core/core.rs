use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use image::{ImageBuffer, Rgb};
use scap::{
    capturer::{self, Capturer},
    targets,
};
use tokio::sync::{Mutex, watch};
use tokio::time;

use crate::{
    core::error,
    engine::{BlackDesertLootTracker, LootData, OCRViaStreamConfig},
    ocr::{self, OcrClient, OcrInput},
};

#[derive(Clone)]
pub struct Core {
    loot_tracker: Arc<Mutex<BlackDesertLootTracker>>,
    screen_capturer: Option<Arc<Mutex<Capturer>>>,
    ocr_client: Arc<OcrClient>,
    loot_sender: watch::Sender<HashMap<String, LootData>>,
    mutex: Arc<Mutex<u8>>,
}

impl Core {
    pub fn new() -> Result<Self, error::Error> {
        let loot_tracker = BlackDesertLootTracker::new();
        let config = loot_tracker.stream_config.clone();
        let capturer = live_capture(config);
        if let Err(capturer) = capturer {
            return Err(error::Error::CapturerError(capturer.to_string()));
        }
        let capturer = capturer.unwrap();
        let ocr_client = OcrClient::new();

        // Create channel for loot data updates (initialized with empty map)
        let (loot_sender, _) = watch::channel(HashMap::new());

        Ok(Self {
            loot_tracker: Arc::new(Mutex::new(loot_tracker)),
            screen_capturer: Some(Arc::new(Mutex::new(capturer))),
            ocr_client: Arc::new(ocr_client),
            loot_sender,
            mutex: Arc::new(Mutex::new(0)),
        })
    }

    pub async fn start(&self) -> Result<(), error::Error> {
        self.recapture_into_exact_frame().await?;

        // Clone self for the background task
        let self_clone = self.clone();

        // Spawn the capture loop as a background task
        tokio::spawn(async move {
            self_clone.run_capture_loop().await;
        });
        Ok(())
    }

    async fn run_capture_loop(&self) {
        let mut empty_frame_count = 0;
        const MAX_EMPTY_FRAMES: usize = 10;
        // const FRAME_INTERVAL: Duration = Duration::from_millis(500);
        {
            self.screen_capturer
                .as_ref()
                .unwrap()
                .lock()
                .await
                .start_capture();
        }
        loop {
            match self.get_data().await {
                Ok(data) => {
                    empty_frame_count = 0;
                    let texts: Vec<String> = data.data.into_iter().map(|f| f.text).collect();

                    // Update shared loot data

                    let _ = self.mutex.lock().await;
                    let mut tracker = self.loot_tracker.lock().await;
                    tracker.insert(&texts);
                    // Send update to all receivers
                    let _ = self.loot_sender.send(tracker.get_loot_data().clone());
                }
                Err(e) => {
                    eprintln!("Error in capture loop: {}", e);
                    empty_frame_count += 1;
                    if empty_frame_count >= MAX_EMPTY_FRAMES {
                        eprintln!("Too many empty frames, stopping capture loop");
                        break;
                    }
                }
            }
            // Add delay between captures to prevent CPU overload
            // time::sleep(FRAME_INTERVAL).await;
        }
    }

    async fn get_data(&self) -> Result<ocr::OcrOutput, error::Error> {
        let frame = {
            let mut capturer = self.screen_capturer.as_ref().unwrap().lock().await;
            capturer.get_next_frame().await
        };

        if let Err(frame) = frame {
            return Err(error::Error::CapturerError(frame.to_string()));
        }
        let frame = frame.unwrap().to_rgb();

        // image::save_buffer(
        //     format!("{}.png", chrono::Local::now().timestamp()),
        //     &frame.data,
        //     frame.width,
        //     frame.height,
        //     image::ExtendedColorType::Rgb8,
        // );
        // let _guard = self.mutex.lock().await;
        self.ocr_client
            .do_ocr(OcrInput {
                data: frame.data,
                width: frame.width,
                height: frame.height,
            })
            .await
            .map_err(|e| error::Error::OcrError(e.to_string()))
    }

    async fn recapture_into_exact_frame(&self) -> Result<(), error::Error> {
        self.screen_capturer
            .as_ref()
            .unwrap()
            .lock()
            .await
            .start_capture();
        let data = self.get_data().await?;
        self.crop(data).await
    }

    async fn crop(&self, input: ocr::OcrOutput) -> Result<(), error::Error> {
        let config = {
            let mut tracker = self.loot_tracker.lock().await;
            let config = tracker.analyze(input.data.into_iter().map(Into::into).collect());
            tracker.stream_config = config.clone();
            config
        };

        let new_capturer = live_capture(config);
        if let Err(new_capturer) = new_capturer {
            return Err(error::Error::CapturerError(new_capturer.to_string()));
        }
        let new_capturer = new_capturer.unwrap();
        {
            let mut capturer = self.screen_capturer.as_ref().unwrap().lock().await;
            capturer.stop_capture();
            *capturer = new_capturer;
        }

        Ok(())
    }

    /// Returns a receiver that will get updates whenever loot data changes
    pub fn get_loot_updates(&self) -> watch::Receiver<HashMap<String, LootData>> {
        self.loot_sender.subscribe()
    }

    /// Gets the current loot data snapshot
    pub async fn get_current_loot(&self) -> HashMap<String, LootData> {
        self.loot_tracker.lock().await.get_loot_data().clone()
    }
}

fn live_capture(
    config: OCRViaStreamConfig,
) -> core::result::Result<capturer::Capturer, error::Error> {
    let search = "BLACK DESERT";
    // // Get recording targets
    let targets = targets::get_all_targets();
    // print!("{:?}", targets);
    let target = targets.iter().find_map(|target| match target {
        targets::Target::Window(x) if x.title.contains(search) => Some(target.clone()),
        targets::Target::Display(x) if x.title.contains(search) => Some(target.clone()),
        _ => None,
    });
    if target.is_none() {
        return Err(error::Error::CapturerError("target not found".to_string()));
    }
    let target = target.unwrap();
    let area = config.capture_area;
    let mut recording_area = area;
    if area.height == 0 && area.width == 0 && area.x == 0 && area.y == 0 {
        let dimension = targets::get_target_dimensions(&target);
        recording_area.width = dimension.0 as u32;
        recording_area.height = dimension.1 as u32;
    }

    let options = capturer::Options {
        fps: config.stream_fps as f32,
        show_cursor: false,
        show_highlight: false,
        excluded_targets: None,
        output_type: scap::frame::FrameType::BGRAFrame,
        output_resolution: scap::capturer::Resolution::_480p,
        crop_area: Some(scap::capturer::Area {
            origin: capturer::Point {
                x: recording_area.x as f64,
                y: recording_area.y as f64,
            },
            size: capturer::Size {
                width: recording_area.width as f64,
                height: recording_area.height as f64,
            },
        }),
        target: Some(target),
        ..Default::default()
    };
    let capturer = scap::capturer::Capturer::build(options)
        .or_else(|x| Err(error::Error::CapturerError(x.to_string())))?;
    Ok(capturer)
}

#[cfg(test)]
mod test_loot_tracker_result {
    use std::io::Cursor;
    use std::{collections::HashMap, fmt::format, sync::Arc};

    use image::codecs::png;
    use image::{ImageEncoder, Rgb};
    use tokio::sync::Mutex;
    use tokio::sync::watch;
    use tokio::time::{self, sleep};

    use crate::{
        core::Core,
        engine::BlackDesertLootTracker,
        ocr::{OcrClient, OcrInput},
    };
    #[tokio::test]
    async fn test_loot_data() {
        // BlackDesertLootTracker
        let loot_tracker = BlackDesertLootTracker::new();
        let ocr_client = OcrClient::new();

        // Create channel for loot data updates (initialized with empty map)
        let (loot_sender, _) = watch::channel(HashMap::new());
        let core = Core {
            loot_sender: loot_sender,
            loot_tracker: Arc::new(Mutex::new(loot_tracker)),
            ocr_client: Arc::new(ocr_client),
            screen_capturer: None,
            mutex: Arc::new(Mutex::new(0)),
        };
        for i in 1..29 {
            let img = image::open(format!("sample ({}).png", i.to_string())).unwrap();
            let img_buffer = img.clone().to_rgb8();
            // let mut png_buffer = Vec::with_capacity(1024 * 1024); // Pre-allocate 1MB
            // _ = image::codecs::png::PngEncoder::new_with_quality(
            //     &mut Cursor::new(&mut png_buffer),
            //     png::CompressionType::Fast,
            //     png::FilterType::NoFilter,
            // )
            // .write_image(
            //     &img_buffer,
            //     img.width(),
            //     img.height(),
            //     image::ExtendedColorType::Rgb8,
            // );

            let data = core
                .ocr_client
                .do_ocr(OcrInput {
                    data: img_buffer.to_vec(),
                    width: img.width(),
                    height: img.height(),
                })
                .await
                .unwrap();
            let texts: Vec<String> = data.data.into_iter().map(|f| f.text).collect();

            // Update shared loot data
            {
                let mut tracker = core.loot_tracker.lock().await;
                tracker.insert(&texts);
                println!(
                    "{}",
                    serde_json::to_string_pretty(&tracker.get_loot_data().clone()).unwrap()
                );
            }
            _ = sleep(time::Duration::from_secs(1)).await;
        }
    }
}
