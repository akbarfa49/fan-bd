use core::fmt;

use std::fs::File;
use std::io::{BufWriter, Write};
use std::ops::{Add, AddAssign, Index, Mul};
use std::os::windows::fs::FileExt;
use std::sync::Arc;

use chrono::*;
use derive_more::From;
use image::math::Rect;
use regex::Regex;
use scap::capturer::Resolution;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{cmp::*, vec};
use tokio::sync::Mutex;
use unicode_normalization::UnicodeNormalization;
#[derive(Clone, Copy)]
pub enum LootDetectionMode {
    OCRChatLootViaStream,
    OCRDropLogViaStream,
}
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct LootData {
    pub id: u64,
    pub name: String,
    pub amount: u64,
    pub price: Silver,
    // add hour and minute to improve accuracy
    pub hour: u8,
    pub minute: u8,
}

impl LootData {
    pub fn calculate(&self) -> Silver {
        self.amount * self.price
    }
}

struct LootDatas(Vec<LootData>);

use std::ops::Deref;

use crate::engine::item_fetcher::{self, ItemFetcher};

impl Deref for LootDatas {
    type Target = Vec<LootData>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl LootDatas {
    fn diff(old: &[LootData], new: &[LootData]) -> Vec<LootData> {
        let mut old_index = 0;
        let mut new_index = 0;
        let now = chrono::Local::now();
        let _h = now.hour();
        let _m = now.minute();
        if new.len() == 0 {
            return vec![];
        }
        if old.len() == 0 {
            // while new_index < new.len() {
            //     let new_item = &new[new_index];
            //     // println!("{}", new_item.hour);
            //     // 179 < 180
            //     if (h * 60) + m == (new_item.hour as u32 * 60) + new_item.minute as u32 {
            //         break;
            //     }
            //     new_index += 1
            // }

            return new[new_index..].to_vec();
        }

        let mut whole_match = false;
        // fail whole match? entirely or partially new data? fallback!
        let mut index_fallback = -1;
        while old_index < old.len() && new_index < new.len() {
            let new_item = &new[new_index];
            let old_item = &old[old_index];
            if new_item.name == old_item.name
                && (old_item.amount == new_item.amount
                    || is_ocr_misread(&old_item.amount.to_string(), &new_item.amount.to_string()))
                && old_item.hour == new_item.hour
                && old_item.minute == new_item.minute
            {
                old_index += 1;
                new_index += 1;
                // has_matching_data = true;
                whole_match = true;
                continue;
            }
            // 1 191
            // 1 112
            // 0 1 -> 0
            // 1 191 2 202
            // 2 101
            // 0 1 2 3
            // 0 0 0 1
            if whole_match {
                whole_match = false;
                index_fallback = if index_fallback == -1 {
                    new_index as i32 - 1
                } else {
                    index_fallback
                };
                new_index -= 1;
            }
            // found old pattern then suddenly no pattern, so we try to find the new pattern and look if it match again
            // if has_matching_data {
            //     has_matching_data = false;
            // }
            old_index += 1;
        }

        if !whole_match && index_fallback > -1 {
            new_index = index_fallback as usize;
        }
        // possibly miss ocr
        // if new_index == 0 {
        //     old_index = 0;
        //     let mut counter = 0;
        //     let mut prediction = 0.0;
        //     while old_index < old.len() && new_index < new.len() {
        //         let old_item = &old[old_index];
        //         let new_item = &new[new_index];
        //         // name should be the same already just maybe missed the amount
        //         if old_item.name == new_item.name {
        //             let rate = strsim::levenshtein(
        //                 &old_item.amount.to_string(),
        //                 &new_item.amount.to_string(),
        //             );
        //             if rate > 0.7 {}
        //         }
        //         counter += 1;
        //         old_index += 1;
        //         new_index += 1;
        //     }
        // }
        // if !has_matching_data {
        //     return vec![];
        // }
        let out = new[new_index..].to_vec();
        return out;
    }
}

#[cfg(test)]
mod test_diff {
    use std::fs::File;

    use serde_json::Deserializer;

    use crate::engine::{LootData, Silver, blackdesert::LootDatas};
    #[test]
    fn test_loot() {
        let old_data = vec![
            LootData {
                name: "Swamp Leaves".to_string(),
                amount: 3,
                ..Default::default()
            },
            LootData {
                name: "Silver".to_string(),
                amount: 116,
                ..Default::default()
            },
            LootData {
                name: "Swamp Leaves".to_string(),
                amount: 3,
                ..Default::default()
            },
        ];
        let new_data = vec![
            LootData {
                name: "Swamp Leaves".to_string(),
                amount: 3,
                ..Default::default()
            },
            LootData {
                name: "Silver".to_string(),
                amount: 92,
                ..Default::default()
            },
            LootData {
                name: "Swamp Leaves".to_string(),
                amount: 1,
                ..Default::default()
            },
        ];
        let diff = LootDatas::diff(&old_data, &new_data);
        println!("{:?}", diff);
        assert_eq!(diff[0].name, "Silver");
        assert_eq!(diff[0].amount, 92);
    }
    #[test]
    fn test_diff_input() {
        let file_reader = File::open("1753492193_old.json").unwrap();
        let old_data: Vec<LootData> = serde_json::from_reader(file_reader).unwrap();
        let file_reader = File::open("1753492193_new.json").unwrap();
        let new_data: Vec<LootData> = serde_json::from_reader(file_reader).unwrap();
        let diff = LootDatas::diff(&old_data, &new_data);
        // println!("diff is {:?}", diff);
        assert_ne!(diff.len(), 0);
    }
}

#[derive(Copy, Clone, Default, Debug, Serialize, Deserialize)]
pub struct Silver(u64);

// Allow Silver * Silver
impl Mul for Silver {
    type Output = Silver;

    fn mul(self, rhs: Silver) -> Silver {
        Silver(self.0 * rhs.0)
    }
}

impl Add for Silver {
    type Output = Silver;
    fn add(self, rhs: Silver) -> Silver {
        Silver(self.0 + rhs.0)
    }
}

impl AddAssign for Silver {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}
// Allow u64 * Silver
impl Mul<Silver> for u64 {
    type Output = Silver;

    fn mul(self, rhs: Silver) -> Silver {
        Silver(self * rhs.0)
    }
}

impl Silver {
    fn string(&self) -> String {
        const UNITS: [&str; 5] = ["", "K", "M", "B", "T"];
        let value = self.0;
        let mut val = value as f64;
        let mut idx = 0;

        while val >= 1000.0 && idx < UNITS.len() - 1 {
            val /= 1000.0;
            idx += 1;
        }

        format!("{:.2}{}", val, UNITS[idx])
    }
    pub fn new(data: u64) -> Self {
        Silver(data)
    }
}

impl fmt::Display for Silver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.string())
    }
}

mod test_silver {
    use crate::engine::Silver;

    #[test]
    fn add_silver() {
        let mut a = Silver::new(1);
        let b = Silver::new(2);
        a += b;
        assert_eq!(a.0, 3);
    }
}
/* todo:
1. add ocr function to struct
2. add stream function so that the data can be determined easily
*/
pub struct BlackDesertLootTracker {
    loot_table: HashMap<String, LootData>,
    loot_history: Arc<Mutex<Vec<LootData>>>,
    loot_entry_tracker: Vec<LootData>,
    // default is OCRViaStream
    pub detection_mode: LootDetectionMode,
    // pub stream_config: OCRViaStreamConfig,
    mutex: Mutex<u8>,
    state: State,
    pub item_fetcher: item_fetcher::Fetcher,
}
#[derive(Debug, Clone)]
pub struct OCRViaStreamConfig {
    pub capture_area: Rect,
    pub stream_fps: f64,
}

pub struct ScreenConfig {
    pub capture_area: Rect,
    pub stream_fps: f64,
}

pub struct Screen {
    pub scale: u16,
    pub height: u32,
    pub width: u32,
}

const DROP_LOG_PANEL_WIDTH: f32 = 0.139;
const DROP_LOG_PANEL_HEIGHT: f32 = 0.157;

pub enum State {
    Start,
    Pause,
    Continue,
}
#[derive(Debug, From)]
pub struct AnalyzeCaptureAreaInput {
    pub text: String,

    // how to calc if from coord. x=left,y=top,w=right-left,h=bottom-top
    pub area: Rect,
}

impl BlackDesertLootTracker {
    pub fn new() -> Self {
        Self {
            loot_table: HashMap::new(),
            loot_entry_tracker: vec![],
            detection_mode: LootDetectionMode::OCRChatLootViaStream,
            state: State::Start,
            mutex: Mutex::new(0),
            loot_history: Arc::new(Mutex::new(Vec::new())),
            item_fetcher: item_fetcher::Fetcher::Default(item_fetcher::DefaultFetcher::new()),
        }
    }
    pub fn set_state(&mut self, state: State) {
        self.state = state
    }
    // the data will keep changing if u need it to be persist please use clone
    pub fn get_loot_data(&self) -> &HashMap<String, LootData> {
        return &self.loot_table;
    }
    pub async fn reset(&mut self) {
        self.loot_entry_tracker.clear();
        let mut history = self.loot_history.lock().await;
        history.clear();
        self.loot_table.clear();
    }
    fn parse_loot_drop_logs(data: &String) -> Option<LootData> {
        // find x from right
        if data.is_empty() {
            return None;
        }
        let mut idx = data.len() - 1;
        let chars: Vec<char> = data.chars().collect();
        while idx != 0 {
            let c: String = chars[idx].to_lowercase().collect();
            if c.nfd().to_string() == "x" {
                break;
            }

            idx -= 1;
        }
        if idx == 0 {
            println!("{}", data);
            return None;
        }
        //

        let (raw_loot, amount_str) = (
            chars[..idx].iter().collect::<String>(),
            chars[idx + 1..].iter().collect::<String>(),
        );
        let loot = to_title_case(&normalize_spaces(&raw_loot));
        let amount = extract_number(&amount_str)?;
        if amount == 0 {
            println!("{}", data);
            return None;
        }
        Some(LootData {
            name: loot,
            amount: amount,
            ..Default::default()
        })
    }

    // why not borrow? i need to modify data
    // why no error? i lazy to bring up error since the data will be empty or not because if it have wrong text pattern will assume it as other log
    pub fn parse_loot(detection_mode: LootDetectionMode, data: &String) -> Option<LootData> {
        match detection_mode {
            LootDetectionMode::OCRDropLogViaStream => {
                return Self::parse_loot_drop_logs(data);
            }
            _ => {}
        }
        let data = normalize_spaces(data);
        // add matching with error distance
        if !data.starts_with("You have obtained") {
            return None;
        }
        // manual because im stupid at regex
        let mut itemname = String::new();
        let mut idx = 0;
        // finding name
        let mut found_name = false;
        let data_chars: Vec<char> = data.chars().collect();
        while idx < data.len() {
            let c = data_chars[idx];
            idx += 1;
            if found_name {
                if c == ']' {
                    break;
                }
                itemname.push(c);
                continue;
            }
            if c == '[' {
                found_name = true;
            }
        }
        if itemname.is_empty() {
            return None;
        }
        itemname = to_title_case(&itemname);
        // finding amount
        let mut amount: u64 = 0;
        while idx < data.len() {
            let c = data_chars[idx];
            if c == '(' {
                // fallback
                idx -= 1;
                break;
            }
            idx += 1;
            let nc = c.to_digit(10);
            if nc.is_none() {
                continue;
            }
            amount = (amount * 10) + nc.unwrap() as u64;
        }
        // default amount is 1.
        if amount == 0 {
            amount = 1
        }
        let mut hour = 0;
        let mut minute = 0;
        // finding hour minute
        let mut find_mode = 0;
        while idx < data.len() {
            let c = data_chars[idx];
            idx += 1;
            if c == ':' {
                find_mode += 1;
                continue;
            }
            let nc = c.to_digit(10);
            if nc.is_none() {
                continue;
            }
            let nc = nc.unwrap();
            match find_mode {
                0 => hour = (hour * 10) + nc,
                1 => minute = (minute * 10) + nc,
                _ => {}
            }
        }
        return Some(LootData {
            id: 0,
            name: itemname,
            amount: amount,
            hour: hour as u8,
            minute: minute as u8,
            ..Default::default()
        });
    }
    pub fn multiple_parse_loot(
        detection_mode: LootDetectionMode,
        data: &Vec<String>,
    ) -> Vec<LootData> {
        let mut loot_datas: Vec<LootData> = Vec::new();
        for (_, v) in data.iter().enumerate() {
            if let Some(loot_data) = Self::parse_loot(detection_mode, v) {
                // println!("{:?}", loot_data);
                loot_datas.push(loot_data);
            }
        }
        loot_datas
    }

    pub async fn insert(&mut self, new_entry: &Vec<String>) -> u16 {
        // println!("inserting loot data??");
        // let _guard = self.mutex.lock();
        let mut new_loot_data_entry = Self::multiple_parse_loot(self.detection_mode, new_entry);
        if new_loot_data_entry.is_empty() {
            match self.detection_mode {
                LootDetectionMode::OCRDropLogViaStream => {
                    self.loot_entry_tracker = vec![];
                }
                _ => {}
            }
            return 0;
        }

        if !self.loot_table.is_empty() {
            for v in new_loot_data_entry.iter_mut() {
                let mut rate = 0.0;
                let clean_lootname: String = v
                    .name
                    .clone()
                    .chars()
                    .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                    .collect();
                for key in self.loot_table.keys() {
                    let cleaned_key: String = key
                        .clone()
                        .chars()
                        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                        .collect();
                    if cleaned_key.starts_with(&clean_lootname)
                        || cleaned_key.ends_with(&clean_lootname)
                    {
                        v.name = key.clone();
                        break;
                    }
                    let result = strsim::normalized_damerau_levenshtein(
                        &cleaned_key.to_lowercase(),
                        &clean_lootname.to_lowercase(),
                    );
                    if result > 0.6 && result > rate {
                        v.name = key.clone();
                        rate = result;
                    }
                }
            }
        }
        let diff_loot_data: Vec<LootData>;
        match self.detection_mode {
            LootDetectionMode::OCRChatLootViaStream => {
                let old_loot = self.loot_entry_tracker.clone();
                diff_loot_data = LootDatas::diff(&old_loot, &new_loot_data_entry);
                if diff_loot_data.len() == 0 {
                    return 0;
                }
                self.loot_entry_tracker = new_loot_data_entry;
            }
            LootDetectionMode::OCRDropLogViaStream => {
                let old_loot = self.loot_entry_tracker.clone();
                if !old_loot.is_empty() {
                    diff_loot_data = LootDatas::diff(&old_loot, &new_loot_data_entry);
                    if diff_loot_data.len() == 0 {
                        return 0;
                    }
                } else {
                    diff_loot_data = new_loot_data_entry.clone();
                }
                self.loot_entry_tracker = new_loot_data_entry;
            }
        }
        // let mut loot_history = self.loot_history;
        // let mut history: Vec<LootData> = Vec::new();
        let mut history = self.loot_history.as_ref().lock().await;
        for v in diff_loot_data.iter() {
            history.push(v.clone());
            let loot_table = self.loot_table.get_mut(&v.name);
            if let Some(entry) = loot_table {
                entry.amount += v.amount;
                continue;
            }
            let loot_metadata = self.find_loot_metadata(&v.name).await;
            if loot_metadata.is_none() {
                continue;
            }
            let metadata = loot_metadata.unwrap();
            let new_loot_data = LootData {
                id: metadata.id,
                price: metadata.price,
                amount: v.amount,
                name: v.name.to_owned(),
                hour: v.hour,
                minute: v.minute,
            };
            // history.push(new_loot_data.clone());
            self.loot_table
                .insert(new_loot_data.name.clone(), new_loot_data);
        }
        return diff_loot_data.len() as u16;
        // let file = File::create(format!(
        //     "dump/{}_raw.txt",
        //     chrono::Local::now().timestamp_millis()
        // ))
        // .unwrap();
        // let mut writer = BufWriter::new(file);
        // for v in new_entry {
        //     _ = writeln!(writer, "{}", v);
        // }
        // let file = File::create(format!(
        //     "dump/{}_history.txt",
        //     chrono::Local::now().timestamp_millis()
        // ))
        // .unwrap();
        // let mut writer = BufWriter::new(file);
        // for v in history.to_vec() {
        //     _ = writeln!(writer, "{}: {}", v.name, v.amount);
        // }
    }
    async fn find_loot_metadata(&self, s: &String) -> Option<Item> {
        let result = self.item_fetcher.get_data_by_name(&s).await;
        if let Err(err) = result {
            println!("{}: {}", s, err);
            return None;
        }
        let result = result.unwrap();
        // match self.item_fetcher {
        //     item_fetcher::Fetcher::Default(f) => f.get_data_by_name(item_name),
        // }
        let price = if result.market_sell_price > 0 {
            Silver(result.market_sell_price)
        } else {
            Silver(result.vendor_sell_price)
        };

        // self.item_fetcher.(
        Some(Item {
            id: result.id,
            name: result.name,
            price: price,
        })
    }
    pub fn analyze(&self, input: Vec<AnalyzeCaptureAreaInput>) -> OCRViaStreamConfig {
        let mut config = OCRViaStreamConfig {
            capture_area: Rect {
                x: 1000,
                y: 400,
                width: 920,
                height: 640,
            },
            stream_fps: 20.0,
        };

        let abs_cmp = |x: &u32, y: &u32| x.cmp(&y);
        let mut is_found = false;
        for (_, v) in input.iter().enumerate() {
            let loot_data: Option<LootData> = Self::parse_loot(self.detection_mode, &v.text);

            if let Some(_) = loot_data {
                // println!("{:?}", loot_data);
                if !is_found {
                    config.capture_area.x = v.area.x;
                    config.capture_area.y = v.area.y;
                    config.capture_area.width = v.area.width;
                    config.capture_area.height = v.area.height;
                    is_found = true;
                }
                config.capture_area.x = min_by(config.capture_area.x, v.area.x, abs_cmp);
                config.capture_area.y = min_by(config.capture_area.y, v.area.y, abs_cmp);
                config.capture_area.width =
                    max_by(config.capture_area.width, v.area.width + v.area.x, abs_cmp);
                config.capture_area.height = max_by(
                    config.capture_area.height,
                    v.area.height + v.area.y,
                    abs_cmp,
                );
            }
        }
        if is_found {
            config.capture_area.height -= config.capture_area.y;
            config.capture_area.width -= config.capture_area.x;
        }
        // expand littlebit
        // if config.capture_area.x > 4 {
        //     config.capture_area.x -= 4;
        //     config.capture_area.width += 4;
        // }
        // if config.capture_area.y > 4 {
        //     config.capture_area.y -= 4;
        //     config.capture_area.height += 4;
        // }
        config
    }

    pub fn screen_config(
        detection_mode: LootDetectionMode,
        input: Vec<AnalyzeCaptureAreaInput>,
        screen: Option<Screen>,
    ) -> ScreenConfig {
        let mut config = ScreenConfig {
            capture_area: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            stream_fps: 20.0,
        };

        match detection_mode {
            LootDetectionMode::OCRDropLogViaStream => {
                let screen = screen.unwrap();
                config.stream_fps = 2.4;

                config.capture_area.width =
                    ((screen.width as f32 * screen.scale as f32 * DROP_LOG_PANEL_WIDTH) / 100.0)
                        .ceil() as u32;

                config.capture_area.height =
                    ((screen.height as f32 * screen.scale as f32 * DROP_LOG_PANEL_HEIGHT) / 100.0
                        * 2.0)
                        .ceil() as u32;
                let text_droplog_center = (1315, 640 as u32);

                config.capture_area.x = text_droplog_center.0 - (config.capture_area.width / 2);
                config.capture_area.y = text_droplog_center.1 - (config.capture_area.height / 2);
                // extend for possibly long name
                config.capture_area.width += 150;
                return config;
            }
            _ => {}
        }

        let abs_cmp = |x: &u32, y: &u32| x.cmp(&y);
        let mut is_found = false;
        for (_, v) in input.iter().enumerate() {
            let loot_data: Option<LootData> = Self::parse_loot(detection_mode, &v.text);

            if let Some(_) = loot_data {
                // println!("{:?}", loot_data);
                if !is_found {
                    config.capture_area.x = v.area.x;
                    config.capture_area.y = v.area.y;
                    config.capture_area.width = v.area.width;
                    config.capture_area.height = v.area.height;
                    is_found = true;
                }
                config.capture_area.x = min_by(config.capture_area.x, v.area.x, abs_cmp);
                config.capture_area.y = min_by(config.capture_area.y, v.area.y, abs_cmp);
                config.capture_area.width =
                    max_by(config.capture_area.width, v.area.width + v.area.x, abs_cmp);
                config.capture_area.height = max_by(
                    config.capture_area.height,
                    v.area.height + v.area.y,
                    abs_cmp,
                );
            }
        }
        if is_found {
            config.capture_area.height -= config.capture_area.y;
            config.capture_area.width -= config.capture_area.x;
        }
        config
    }
}

struct Item {
    id: u64,
    name: String,
    price: Silver,
}

fn normalize_spaces(input: &str) -> String {
    let mut output = String::new();
    let mut was_whitespace = false;

    for c in input.trim().chars() {
        if c.is_whitespace() {
            if !was_whitespace {
                output.push(' ');
                was_whitespace = true;
            }
        } else {
            output.push(c);
            was_whitespace = false;
        }
    }

    output
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_normal() {
//         let loot_data = BlackDesertLootTracker::parse_loot(
//             LootDetectionMode::OCRChatLootViaStream,
//             &"You have obtained [Black Stone]x7. (16:08)".to_string(),
//         )
//         .unwrap();
//         assert_eq!(loot_data.name, "Black Stone");
//     }
//     #[test]
//     fn test_multiple_spaces() {
//         let loot_data = BlackDesertLootTracker::parse_loot(
//             LootDetectionMode::OCRChatLootViaStream,
//             &"You have obtained  [Black Stone]x7. (16:08)".to_string(),
//         )
//         .unwrap();
//         assert_eq!(loot_data.name, "Black Stone");
//         assert_eq!(loot_data.amount, 7);
//     }
//     #[test]
//     fn test_no_time() {
//         let loot_data = BlackDesertLootTracker::parse_loot(
//             LootDetectionMode::OCRChatLootViaStream,
//             &"You have obtained  [Black Stone]x7.".to_string(),
//         )
//         .unwrap();
//         assert_eq!(loot_data.name, "Black Stone");
//         assert_eq!(loot_data.amount, 7);
//     }
//     #[test]
//     fn test_no_amount() {
//         let loot_data = BlackDesertLootTracker::parse_loot(
//             LootDetectionMode::OCRChatLootViaStream,
//             &"You have obtained  [Black Stone].(16:08)".to_string(),
//         )
//         .unwrap();
//         assert_eq!(loot_data.name, "Black Stone");
//         assert_eq!(loot_data.amount, 1);
//         assert_eq!(loot_data.hour, 16)
//     }
//     #[test]
//     fn test_no_amount_time() {
//         let loot_data = BlackDesertLootTracker::parse_loot(
//             LootDetectionMode::OCRChatLootViaStream,
//             &"You have obtained  [Black Stone].(16:08)".to_string(),
//         )
//         .unwrap();
//         assert_eq!(loot_data.name, "Black Stone");
//         assert_eq!(loot_data.amount, 1);
//     }
//     #[test]
//     fn test_title_case() {
//         let t1 = "title".to_string();
//         let t2 = "title Title title";
//         assert_eq!(to_title_case(&t1), "Title");
//         assert_eq!(to_title_case(&t2), "Title Title Title");
//     }
//     #[test]
//     fn test_flow() {
//         let mut tracker = BlackDesertLootTracker::new();
//         tracker.detection_mode = LootDetectionMode::OCRDropLogViaStream;

//         tracker.loot_entry_tracker.push(LootData {
//             id: 0,
//             name: "Swamp Leaves".to_string(),
//             amount: 2,
//             price: Silver(0),
//             hour: 0,
//             minute: 0,
//         });
//         tracker.loot_entry_tracker.push(LootData {
//             id: 0,
//             name: "Silver".to_string(),
//             amount: 111,
//             price: Silver(0),
//             hour: 0,
//             minute: 0,
//         });
//         tracker.loot_table.insert(
//             "Silver".to_string(),
//             LootData {
//                 name: "Silver".to_string(),
//                 amount: 348,
//                 ..Default::default()
//             },
//         );
//         tracker.loot_table.insert(
//             "Swamp Leaves".to_string(),
//             LootData {
//                 name: "Swamp Leaves".to_string(),
//                 amount: 3,
//                 ..Default::default()
//             },
//         );
//         {
//             *tracker.loot_history.lock().unwrap() = tracker.loot_entry_tracker.clone();
//         }
//         let input: Vec<String> = vec![
//             "SWarnp Leaves x 2".to_string(),
//             "CRITICAL".to_string(),
//             "Silver x 109".to_string(),
//             "Swamp Leaves x 2".to_string(),
//         ];
//         tracker.insert(&input);
//         let history = tracker.loot_history.lock().unwrap();
//         println!("{:?}", tracker.loot_table);
//         println!("{:?}", history);
//         assert_eq!(history.len(), 6);
//     }
// }
fn to_title_case(s: &str) -> String {
    s.trim()
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first
                    .to_uppercase()
                    .chain(chars.flat_map(|c| c.to_lowercase()))
                    .collect(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

// #[cfg(test)]
// mod parse_test {
//     use crate::engine::BlackDesertLootTracker;

//     #[test]
//     fn drop_log_parse_test() {
//         let lootdata1 =
//             BlackDesertLootTracker::parse_loot_drop_logs(&"Silverx100".to_string()).unwrap();
//         assert_eq!(lootdata1.name, "Silver");
//         assert_eq!(lootdata1.amount, 100);
//         let lootdata2 =
//             BlackDesertLootTracker::parse_loot_drop_logs(&"silver x100".to_string()).unwrap();
//         assert_eq!(lootdata2.name, "Silver");
//         assert_eq!(lootdata2.amount, 100);
//         let lootdata3 =
//             BlackDesertLootTracker::parse_loot_drop_logs(&" Swamp Leaves x 1".to_string()).unwrap();
//         assert_eq!(lootdata3.name, "Swamp Leaves");
//         assert_eq!(lootdata3.amount, 1);
//     }
// }
fn is_ocr_misread(expected: &str, actual: &str) -> bool {
    actual.len() < expected.len() && expected.starts_with(actual)
}

fn extract_number(s: &str) -> Option<u64> {
    let bytes = s.as_bytes();
    let mut num = 0;
    let mut found = false;

    for &b in bytes {
        if b.is_ascii_digit() {
            found = true;
            num = num * 10 + (b - b'0') as u64;
        } else if found {
            break; // Stop after the first numeric sequence
        }
    }

    found.then_some(num)
}
