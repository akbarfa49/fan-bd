// use anyhow::Ok;
// use core::error;
use image::{Rgb, RgbImage};
use imageproc::drawing::draw_hollow_rect_mut;
use imageproc::rect::Rect;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::io::{BufWriter, Write};
use std::sync::Arc;
use std::{collections::HashMap, fs::File};
use tokio::sync::{Mutex, mpsc, watch};
use tokio::time;

use crate::ocr::OcrOutput;
use crate::{
    core::{IFrameCapturer, capturer, error, game_screen},
    engine::{BlackDesertLootTracker, LootData, LootDetectionMode, Screen},
    ocr::{self, OcrClient, OcrInput},
};

#[derive(Clone)]
enum CoreStatus {
    Initiated,
    Started,
    Stopped,
}

#[derive(Clone)]
pub struct Core {
    loot_tracker: Arc<Mutex<BlackDesertLootTracker>>,
    ocr_client: Arc<OcrClient>,
    loot_sender: watch::Sender<HashMap<String, LootData>>,
    capturer: Option<Arc<Mutex<scap::capturer::Capturer>>>,
    pub game_screen: GameScreen,
    status: Arc<Mutex<CoreStatus>>,
}

#[derive(Clone, Copy)]
pub struct GameScreen {
    pub height: u32,
    pub width: u32,
    // 100% = 100
    pub scale: u16,
}
struct OcrChannel {
    index: u64,
    result: Option<OcrOutput>,
    err: Option<error::Error>,
}

impl Core {
    pub fn new() -> Result<Self, error::Error> {
        let loot_tracker = BlackDesertLootTracker::new();
        // let config = loot_tracker.stream_config.clone();
        // let capturer = live_capture(config);
        // if let Err(capturer) = capturer {
        //     return Err(error::Error::CapturerError(capturer.to_string()));
        // }
        // let capturer = capturer.unwrap();
        let ocr_client = OcrClient::new();

        // Create channel for loot data updates (initialized with empty map)
        let (loot_sender, _) = watch::channel(HashMap::new());
        let game_screen = game_screen()?;
        Ok(Self {
            loot_tracker: Arc::new(Mutex::new(loot_tracker)),
            ocr_client: Arc::new(ocr_client),
            loot_sender,
            // mutex: Arc::new(Mutex::new(0)),
            capturer: None,
            game_screen: game_screen,
            status: Arc::new(Mutex::new(CoreStatus::Initiated)),
        })
    }
    pub fn default() {}
    pub async fn use_chatlog(&mut self) {
        let mut tracker = self.loot_tracker.lock().await;
        tracker.detection_mode = LootDetectionMode::OCRChatLootViaStream
    }
    pub async fn use_drop(&mut self) {
        let mut tracker = self.loot_tracker.lock().await;
        tracker.detection_mode = LootDetectionMode::OCRDropLogViaStream
    }
    pub async fn start(&mut self) -> Result<(), error::Error> {
        // recapture into exact frame first
        self.recapture_into_exact_frame().await?;

        // Clone self for the background task
        let self_clone = self.clone();

        // Spawn the capture loop as a background task
        tokio::spawn(async move {
            self_clone.run_capture_loop().await;
        });
        let mut status = self.status.as_ref().lock().await;
        *status = CoreStatus::Started;
        Ok(())
    }
    pub async fn stop(&mut self) {
        self.capturer.as_ref().unwrap().lock().await.stop_capture();
        self.loot_tracker.lock().await.reset().await;
    }

    async fn run_capture_loop(&self) {
        // let mut empty_frame_count = 0;
        // const MAX_EMPTY_FRAMES: usize = 10;
        // const FRAME_INTERVAL: Duration = Duration::from_millis(500);
        // game_screen();
        {
            self.capturer.as_ref().unwrap().lock().await.start_capture();
        }
        let (sender, mut receiver) = mpsc::channel::<OcrChannel>(10);
        let get_data_channel = self.clone();
        tokio::spawn(async move {
            _ = get_data_channel.get_data_channel(sender).await;
        });
        let mut idx = 0;
        let mut ordered_buffer: HashMap<u64, OcrChannel> = HashMap::new();
        loop {
            tokio::select! {
                Some(msg) = receiver.recv() => {
                    if msg.index != idx{
                        ordered_buffer.insert(msg.index, msg);
                        continue
                    }


                     self.process_data(msg).await;

                    idx +=1;
                    loop{
                        let data = ordered_buffer.remove(&idx);
                        if data.is_none(){
                            break
                        }

                        self.process_data(data.unwrap()).await;
                        idx += 1;
                    }

                },
                _ = time::sleep(time::Duration::from_secs(1)) => {
                let status = self.status.lock().await;
                match *status {
                    CoreStatus::Stopped => break,
                    _ => {}
                };

                }
            }

            // Add delay between captures to prevent CPU overload
            // time::sleep(FRAME_INTERVAL).await;
        }
        drop(receiver);
    }

    async fn process_data(&self, input: OcrChannel) {
        if input.result.is_none() {
            return;
        }
        let data = input.result.unwrap();

        let texts: Vec<String> = data.data.into_iter().map(|f| f.text).collect();
        // let file = File::create(format!(
        //     "{}_history.txt",
        //     chrono::Local::now().timestamp_millis()
        // ))
        // .unwrap();
        // let mut writer = BufWriter::new(file);
        // for v in &texts {
        //     _ = writeln!(writer, "{}", v);
        // }
        // Update shared loot data

        // let _ = self.mutex.lock().await;

        let mut tracker = self.loot_tracker.lock().await;
        tracker.insert(&texts).await;
        // Send update to all receivers
        let _ = self.loot_sender.send(tracker.get_loot_data().clone());
    }

    async fn get_data(&self) -> Result<ocr::OcrOutput, error::Error> {
        let frame = {
            // println!("geting capturer");
            let mut capturer = self.capturer.as_ref().unwrap().lock().await;
            let frame = capturer.get_next_frame().await;
            // println!("frame fetched");
            frame
        };

        if let Err(frame) = frame {
            return Err(error::Error::CapturerError(frame.to_string()));
        }
        let frame = frame.unwrap().to_rgb();
        // let frame = self
        //     .capturer
        //     .as_ref()
        //     .unwrap()
        //     .lock()
        //     .await
        //     .g()
        //     .await;
        // if let Err(frame) = frame {
        //     return Err(frame);
        // }
        // let frame = frame.unwrap();
        // image::save_buffer(
        //     format!("{}.png", chrono::Local::now().timestamp()),
        //     &frame.data,
        //     frame.width,
        //     frame.height,
        //     image::ExtendedColorType::Rgb8,
        // );
        // let _guard = self.mutex.lock().await;
        let result = self
            .ocr_client
            .do_ocr(OcrInput {
                data: frame.data.clone(),
                width: frame.width,
                height: frame.height,
            })
            .await
            .map_err(|e| error::Error::OcrError(e.to_string()));
        if result.is_err() {
            return result;
        }
        let output = result.unwrap();
        // let mut img = RgbImage::from_raw(frame.width, frame.height, frame.data).unwrap();
        // for v in output.data.iter() {
        //     let rect =
        //         Rect::at(v.area.x as i32, v.area.y as i32).of_size(v.area.width, v.area.height);
        //     draw_hollow_rect_mut(&mut img, rect, Rgb([0, 255, 0]));
        // }
        // img.save(format!("{}.png", chrono::Local::now().timestamp_millis()));
        return Ok(output);
    }
    async fn get_data_channel(&self, sender: mpsc::Sender<OcrChannel>) -> Result<(), error::Error> {
        // let frame = self
        //     .capturer
        //     .as_ref()
        //     .unwrap()
        //     .lock()
        //     .await
        //     .g()
        //     .await;
        // if let Err(frame) = frame {
        //     return Err(frame);
        // }
        // let frame = frame.unwrap();

        // let _guard = self.mutex.lock().await;
        let mut idx = 0;
        loop {
            {
                let status = self.status.lock().await;
                match *status {
                    CoreStatus::Stopped => break,
                    _ => {}
                };
            }

            let frame = {
                // println!("geting capturer");
                let mut capturer = self.capturer.as_ref().unwrap().lock().await;
                let frame = capturer.get_next_frame().await;
                // println!("frame fetched");
                frame
            };

            if let Err(frame) = frame {
                return Err(error::Error::CapturerError(frame.to_string()));
            }
            let frame = frame.unwrap().to_rgb();
            let ocr_client = self.ocr_client.clone();
            let cloned_sender = sender.clone();
            // _ = image::save_buffer(
            //     format!("{}.png", chrono::Local::now().timestamp()),
            //     &frame.data,
            //     frame.width,
            //     frame.height,
            //     image::ExtendedColorType::Rgb8,
            // );
            tokio::spawn(async move {
                let result = ocr_client
                    .do_ocr(OcrInput {
                        data: frame.data.clone(),
                        width: frame.width,
                        height: frame.height,
                    })
                    .await
                    .map_err(|e| error::Error::OcrError(e.to_string()));
                if result.is_err() {
                    // return result;
                    _ = cloned_sender
                        .send(OcrChannel {
                            index: idx,
                            result: None,
                            err: result.err(),
                        })
                        .await;
                    return;
                }

                _ = cloned_sender
                    .send(OcrChannel {
                        index: idx,
                        result: Some(result.unwrap()),
                        err: None,
                    })
                    .await;
            });
            idx += 1;
        }

        // let mut img = RgbImage::from_raw(frame.width, frame.height, frame.data).unwrap();
        // for v in output.data.iter() {
        //     let rect =
        //         Rect::at(v.area.x as i32, v.area.y as i32).of_size(v.area.width, v.area.height);
        //     draw_hollow_rect_mut(&mut img, rect, Rgb([0, 255, 0]));
        // }
        // img.save(format!("{}.png", chrono::Local::now().timestamp_millis()));
        return Ok(());
    }

    async fn recapture_into_exact_frame(&self) -> Result<(), error::Error> {
        self.capturer.as_ref().unwrap().lock().await.start_capture();
        let data = self.get_data().await?;
        self.crop(data).await
    }

    async fn crop(&self, input: ocr::OcrOutput) -> Result<(), error::Error> {
        let config = {
            let tracker = self.loot_tracker.lock().await;
            let config = BlackDesertLootTracker::screen_config(
                tracker.detection_mode,
                input.data.into_iter().map(Into::into).collect(),
                Some(Screen {
                    height: self.game_screen.height,
                    width: self.game_screen.width,
                    scale: self.game_screen.scale,
                }),
            );
            config
        };

        {
            let mut capturer = self.capturer.as_ref().unwrap().lock().await;
            capturer.stop_capture();
            let new_capturer = capturer::live_capture(config).unwrap();
            *capturer = new_capturer
        }
        // println!("crop done");
        // self.game_screen
        Ok(())
    }
    pub fn use_capturer(&mut self, capturer: scap::capturer::Capturer) {
        self.capturer = Some(Arc::new(Mutex::new(capturer)));
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

// #[cfg(test)]
// mod test_loot_tracker_result {
//     use std::io::Cursor;
//     use std::{collections::HashMap, fmt::format, sync::Arc};

//     use image::codecs::png;
//     use image::{ImageEncoder, Rgb};
//     use tokio::sync::Mutex;
//     use tokio::sync::watch;
//     use tokio::time::{self, sleep};

//     use crate::{
//         core::Core,
//         engine::BlackDesertLootTracker,
//         ocr::{OcrClient, OcrInput},
//     };
//     #[tokio::test]
//     async fn test_loot_data() {
//         // BlackDesertLootTracker
//         let loot_tracker = BlackDesertLootTracker::new();
//         let ocr_client = OcrClient::new();

//         // Create channel for loot data updates (initialized with empty map)
//         let (loot_sender, _) = watch::channel(HashMap::new());
//         let core = Core {
//             loot_sender: loot_sender,
//             loot_tracker: Arc::new(Mutex::new(loot_tracker)),
//             ocr_client: Arc::new(ocr_client),
//             screen_capturer: None,
//             mutex: Arc::new(Mutex::new(0)),
//         };
//         for i in 1..29 {
//             let img = image::open(format!("sample ({}).png", i.to_string())).unwrap();
//             let img_buffer = img.clone().to_rgb8();
//             // let mut png_buffer = Vec::with_capacity(1024 * 1024); // Pre-allocate 1MB
//             // _ = image::codecs::png::PngEncoder::new_with_quality(
//             //     &mut Cursor::new(&mut png_buffer),
//             //     png::CompressionType::Fast,
//             //     png::FilterType::NoFilter,
//             // )
//             // .write_image(
//             //     &img_buffer,
//             //     img.width(),
//             //     img.height(),
//             //     image::ExtendedColorType::Rgb8,
//             // );

//             let data = core
//                 .ocr_client
//                 .do_ocr(OcrInput {
//                     data: img_buffer.to_vec(),
//                     width: img.width(),
//                     height: img.height(),
//                 })
//                 .await
//                 .unwrap();
//             let texts: Vec<String> = data.data.into_iter().map(|f| f.text).collect();

//             // Update shared loot data
//             {
//                 let mut tracker = core.loot_tracker.lock().await;
//                 tracker.insert(&texts);
//                 println!(
//                     "{}",
//                     serde_json::to_string_pretty(&tracker.get_loot_data().clone()).unwrap()
//                 );
//             }
//             _ = sleep(time::Duration::from_secs(1)).await;
//         }
//     }
// }
