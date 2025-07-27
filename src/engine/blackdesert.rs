use std::fmt::Write;
use std::fs::File;
use std::ops::Mul;

use chrono::*;
use derive_more::From;
use image::math::Rect;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{cmp::*, fmt, vec};
enum LootDetectionMode {
    OCRViaStream,
}
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct LootData {
    pub id: u16,
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

impl Deref for LootDatas {
    type Target = Vec<LootData>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl LootDatas {
    fn diff(&self, new: &[LootData]) -> Vec<LootData> {
        let old = self;
        let mut old_index = 0;
        let mut new_index = 0;
        let now = chrono::Local::now();
        let h = now.hour();
        let m = now.minute();
        if new.len() == 0 {
            return vec![];
        }
        if old.len() == 0 {
            while new_index < new.len() {
                let new_item = &new[new_index];
                // println!("{}", new_item.hour);
                // 179 < 180
                if (h * 60) + m == (new_item.hour as u32 * 60) + new_item.minute as u32 {
                    break;
                }
                new_index += 1
            }
            return new[new_index..].to_vec();
        }

        let mut has_matching_data = false;
        while old_index < old.len() && new_index < new.len() {
            let new_item = &new[new_index];
            let old_item = &old[old_index];
            if new_item.name == old_item.name
                && new_item.amount == old_item.amount
                && old_item.hour == new_item.hour
                && old_item.minute == new_item.minute
            {
                old_index += 1;
                new_index += 1;
                has_matching_data = true;
                continue;
            }
            // found old pattern then suddenly no pattern, so we try to find the new pattern and look if it match again
            if has_matching_data {
                has_matching_data = false;
            }
            old_index += 1;
        }
        if !has_matching_data {
            return vec![];
        }
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
                amount: 2,
                ..Default::default()
            },
            LootData {
                name: "Silver".to_string(),
                amount: 110,
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
                name: "Silver".to_string(),
                amount: 110,
                ..Default::default()
            },
            LootData {
                name: "Swamp Leaves".to_string(),
                amount: 3,
                ..Default::default()
            },
            LootData {
                name: "Silver".to_string(),
                amount: 124,
                ..Default::default()
            },
        ];
        let diff = LootDatas(old_data).diff(&new_data);
        println!("{:?}", diff);
        assert_eq!(diff[0].name, "Silver");
    }
    #[test]
    fn test_diff_input() {
        let file_reader = File::open("1753492193_old.json").unwrap();
        let old_data: Vec<LootData> = serde_json::from_reader(file_reader).unwrap();
        let file_reader = File::open("1753492193_new.json").unwrap();
        let new_data: Vec<LootData> = serde_json::from_reader(file_reader).unwrap();
        let diff = LootDatas(old_data).diff(&new_data);
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

/* todo:
1. add ocr function to struct
2. add stream function so that the data can be determined easily
*/
pub struct BlackDesertLootTracker {
    loot_table: HashMap<String, LootData>,

    loot_entry_tracker: Vec<LootData>,
    // default is OCRViaStream
    detection_mode: LootDetectionMode,
    pub stream_config: OCRViaStreamConfig,
    state: State,
}
#[derive(Debug, Clone)]
pub struct OCRViaStreamConfig {
    pub capture_area: Rect,
    pub stream_fps: f64,
}
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
            detection_mode: LootDetectionMode::OCRViaStream,
            stream_config: OCRViaStreamConfig {
                capture_area: Rect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                stream_fps: 0.5,
            },
            state: State::Start,
        }
    }
    pub fn set_state(&mut self, state: State) {
        self.state = state
    }
    // the data will keep changing if u need it to be persist please use clone
    pub fn get_loot_data(&self) -> &HashMap<String, LootData> {
        return &self.loot_table;
    }
    // why not borrow? i need to modify data
    // why no error? i lazy to bring up error since the data will be empty or not because if it have wrong text pattern will assume it as other log
    pub fn parse_loot(data: &String) -> Option<LootData> {
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
    pub fn multiple_parse_loot(data: &Vec<String>) -> Vec<LootData> {
        let mut loot_datas: Vec<LootData> = Vec::new();
        for (_, v) in data.iter().enumerate() {
            if let Some(loot_data) = Self::parse_loot(v) {
                // println!("{:?}", loot_data);
                loot_datas.push(loot_data);
            }
        }
        loot_datas
    }

    pub fn insert(&mut self, new_entry: &Vec<String>) {
        // println!("inserting loot data??");

        let new_loot_data_entry = Self::multiple_parse_loot(new_entry);
        if new_loot_data_entry.is_empty() {
            return;
        }
        let old_loot = self.loot_entry_tracker.clone();
        let diff_loot_data = LootDatas(old_loot.clone()).diff(&new_loot_data_entry);
        if diff_loot_data.len() == 0 {
            return;
        }
        self.loot_entry_tracker = new_loot_data_entry;
        for v in diff_loot_data.iter() {
            let loot_table = self.loot_table.get_mut(&v.name);
            if let Some(entry) = loot_table {
                entry.amount += v.amount;
                continue;
            }
            let loot_metadata = Self::find_loot_metadata(&v.name);
            if loot_metadata.is_none() {
                continue;
            }
            let metadata = loot_metadata.unwrap();
            let new_loot_data = LootData {
                id: metadata.id,
                price: metadata.price,
                amount: v.amount,
                name: metadata.name,
                hour: v.hour,
                minute: v.minute,
            };
            self.loot_table
                .insert(new_loot_data.name.clone(), new_loot_data.clone());
        }
    }
    fn find_loot_metadata(s: &String) -> Option<Item> {
        Some(Item {
            name: s.clone(),
            id: 0,
            price: Silver(1_000_000),
        })
    }
    pub fn analyze(&self, input: Vec<AnalyzeCaptureAreaInput>) -> OCRViaStreamConfig {
        let mut config = OCRViaStreamConfig {
            capture_area: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            stream_fps: 10.0,
        };

        let abs_cmp = |x: &u32, y: &u32| x.cmp(&y);
        let mut is_first = true;
        for (_, v) in input.iter().enumerate() {
            let loot_data: Option<LootData> = Self::parse_loot(&v.text);
            if let Some(_) = loot_data {
                if is_first {
                    config.capture_area.x = v.area.x;
                    config.capture_area.y = v.area.y;
                    config.capture_area.width = v.area.width;
                    config.capture_area.height = v.area.height;
                    is_first = false;
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
        config.capture_area.height -= config.capture_area.y;
        config.capture_area.width -= config.capture_area.x;

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
}

struct Item {
    id: u16,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal() {
        let loot_data = BlackDesertLootTracker::parse_loot(
            &"You have obtained [Black Stone]x7. (16:08)".to_string(),
        )
        .unwrap();
        assert_eq!(loot_data.name, "Black Stone");
    }
    #[test]
    fn test_multiple_spaces() {
        let loot_data = BlackDesertLootTracker::parse_loot(
            &"You have obtained  [Black Stone]x7. (16:08)".to_string(),
        )
        .unwrap();
        assert_eq!(loot_data.name, "Black Stone");
        assert_eq!(loot_data.amount, 7);
    }
    #[test]
    fn test_no_time() {
        let loot_data =
            BlackDesertLootTracker::parse_loot(&"You have obtained  [Black Stone]x7.".to_string())
                .unwrap();
        assert_eq!(loot_data.name, "Black Stone");
        assert_eq!(loot_data.amount, 7);
    }
    #[test]
    fn test_no_amount() {
        let loot_data = BlackDesertLootTracker::parse_loot(
            &"You have obtained  [Black Stone].(16:08)".to_string(),
        )
        .unwrap();
        assert_eq!(loot_data.name, "Black Stone");
        assert_eq!(loot_data.amount, 1);
        assert_eq!(loot_data.hour, 16)
    }
    #[test]
    fn test_no_amount_time() {
        let loot_data = BlackDesertLootTracker::parse_loot(
            &"You have obtained  [Black Stone].(16:08)".to_string(),
        )
        .unwrap();
        assert_eq!(loot_data.name, "Black Stone");
        assert_eq!(loot_data.amount, 1);
    }
    #[test]
    fn test_title_case() {
        let t1 = "title".to_string();
        let t2 = "title Title title";
        assert_eq!(to_title_case(&t1), "Title");
        assert_eq!(to_title_case(&t2), "Title Title Title");
    }
}
fn to_title_case(s: &str) -> String {
    s.split_whitespace()
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
