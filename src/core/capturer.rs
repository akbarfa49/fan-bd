use std::sync::Arc;

use image::math::Rect;
use scap::{
    capturer::{self, Capturer},
    targets,
};
use tokio::sync::Mutex;

use crate::{
    core::{GameScreen, error::Error},
    engine::{self, ScreenConfig},
};
use scap::frame::{self};

pub fn game_screen() -> Result<GameScreen, Error> {
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
        return Err(Error::CapturerError("target not found".to_string()));
    }
    let target = target.unwrap();
    let dimension = targets::get_target_dimensions(&target);
    Ok(GameScreen {
        width: dimension.0 as u32,
        height: dimension.1 as u32,
        scale: 100,
    })
}

pub fn live_capture(config: engine::ScreenConfig) -> Result<capturer::Capturer, Error> {
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
        return Err(Error::CapturerError("target not found".to_string()));
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
        output_resolution: scap::capturer::Resolution::_1080p,
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
        .or_else(|x| Err(Error::CapturerError(x.to_string())))?;
    Ok(capturer)
}

// #[derive(Clone)]
// pub struct FrameCapturer {
//     pub get_frame: fn() -> Result<frame::RGBFrame, Error>,
//     pub stop: fn(),
//     pub start: fn() -> Result<(), Error>,
//     pub recapture: fn(x: u32, y: u32, width: u32, height: u32, fps: f64) -> Result<(), Error>,
// }

#[async_trait::async_trait]
pub trait IFrameCapturer: Send + Sync {
    fn get_frame(&mut self) -> impl Future<Output = Result<frame::RGBFrame, Error>> + Send;
    fn stop(&mut self) -> impl Future<Output = ()>;
    fn start(&mut self) -> impl Future<Output = Result<(), Error>>;
    fn config(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        fps: f64,
    ) -> impl Future<Output = Result<(), Error>>;
}

impl IFrameCapturer for Arc<Mutex<capturer::Capturer>> {
    async fn get_frame(&mut self) -> Result<frame::RGBFrame, Error> {
        match self.lock().await.get_next_frame().await {
            Ok(f) => return Ok(f.to_rgb()),
            Err(e) => return Err(Error::CapturerError(e.to_string())),
        };
    }
    async fn stop(&mut self) {
        self.as_ref().lock().await.stop_capture();
    }
    async fn start(&mut self) -> Result<(), Error> {
        self.as_ref().lock().await.start_capture();
        Ok(())
    }
    async fn config(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        fps: f64,
    ) -> Result<(), Error> {
        let config = ScreenConfig {
            capture_area: Rect {
                x: x,
                y: y,
                width: width,
                height: height,
            },
            stream_fps: fps,
        };
        let new_capturer = live_capture(config).map_err(|e| Error::CapturerError(e.to_string()))?;
        let mut capturer = self.as_ref().lock().await;
        *capturer = new_capturer;
        Ok(())
    }
}

pub fn config(x: u32, y: u32, width: u32, height: u32, fps: f64) -> Result<Capturer, Error> {
    let config = ScreenConfig {
        capture_area: Rect {
            x: x,
            y: y,
            width: width,
            height: height,
        },
        stream_fps: fps,
    };
    let new_capturer = live_capture(config).map_err(|e| Error::CapturerError(e.to_string()))?;
    Ok(new_capturer)
}
