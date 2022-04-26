extern crate easy_http_request;
extern crate chrono;
extern crate regex;
extern crate serde;
extern crate json;
use serde::{Serialize, Deserialize};
use std::process::Command;
use std::path::{PathBuf, Path};
use std::io::Write;
use std::io::Read;
use std::vec::Vec;
use std::str;

#[derive(Serialize, Deserialize, Debug, Clone)]
enum ChannelType {
    Channel,
    User,
    C
}

#[derive(Debug)]
pub struct Video {
    pub video_title: String,
    pub video_id: String,
    pub video_desc: String,
    pub is_live: bool,
    pub tags: Option<Vec<String>>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Channel {
    pub name: String,
    pub channel_id: String,
    channel_type: ChannelType,
    filter: Vec<String>,
    path: PathBuf,
    pub pic_path: PathBuf,
    latest_ids: (Option<String>, Option<String>)
}

impl Channel {

    // Create a new Channel
    pub fn new(channel_name: String, channel_url: String, base_path: &PathBuf, filter_words: Vec<String>) -> Result<Channel, ()> {

        // Throw a tantrum if any of the args are empty
        if &channel_name == "" || &channel_url == "" { return Err(()); }

        // Split the URL
        let split_url: Vec<_> = channel_url.split("/").into_iter().collect();

        // Parse the channel type
        let channel_type = {
            if split_url.iter().any(|&i| i == "channel") { ChannelType::Channel }
            else if split_url.iter().any(|&i| i == "user") { ChannelType::User }
            else if split_url.iter().any(|&i| i == "c") { ChannelType::C }
            else { return Err(()); }
        };

        // Get the actual ID
        let id = match channel_type {
            ChannelType::Channel => { split_url.get(split_url.iter().position(|&r| r == "channel").unwrap()+1).unwrap() },
            ChannelType::User => {  split_url.get(split_url.iter().position(|&r| r == "user").unwrap()+1).unwrap()  },
            ChannelType::C => {  split_url.get(split_url.iter().position(|&r| r == "c").unwrap()+1).unwrap()  }
        };

        // Set up the paths
        let mut cfg_path = base_path.clone();
        cfg_path.push(format!("{}.json", id));
        if let Err(_) = std::fs::File::create(&cfg_path) { return Err(()); }

        let mut pic_path = base_path.clone();
        pic_path.push("icons");
        pic_path.push(format!("{}.png", id));
        

        // Set the actual channel values
        let mut ret_channel = Channel {
            name: channel_name,
            channel_id: String::from(*id),
            channel_type: channel_type,
            filter: filter_words,
            path: cfg_path,
            pic_path: pic_path,
            latest_ids: (None, None)
        };

        // If it's a C type channel, get the true ID and assign the latest video id
        if let ChannelType::C = ret_channel.channel_type {
            if let Err(_) = ret_channel.get_true_channel() {
                // For some reason, we couldn't update the channel ID
                return Err(());
            }
        }

        let _ = ret_channel.write_channel_to_file();
        
        // Update the latest ids
        if let (Ok(latest_found_id_1), Ok(latest_found_id_2)) = (ret_channel.get_vid_id_from_index(0), ret_channel.get_vid_id_from_index(1)) {
            ret_channel.latest_ids = (Some(latest_found_id_1), Some(latest_found_id_2));
            Ok(ret_channel)
        } else { Err(()) }

    } // end new

    // Get a channel from file
    pub fn from_file(file: &Path) -> Result<Channel, ()> {
        let mut saved_file = std::fs::File::open(file).unwrap();
        let mut buffer = String::new();

        if let Ok(_) = saved_file.read_to_string(&mut buffer) {
            // File was read
            let ret_channel_res: Result<Channel, _> = serde_json::from_str(buffer.as_str());
            if let Ok(ret_channel) = ret_channel_res {
                Ok(ret_channel)
            } else { Err(()) }
        } else { Err(()) }
    }

    // Get the request url
    fn get_feed_url(&self) -> String {
        match self.channel_type {
            ChannelType::Channel => { format!("https://www.youtube.com/channel/{}/videos", self.channel_id) },
            ChannelType::User => { format!("https://www.youtube.com/user/{}/videos", self.channel_id) },
            ChannelType::C => { format!("https://www.youtube.com/c/{}/videos", self.channel_id) }
        }
    } // end get_req_url

    // Get the request url
    /*fn get_vids_url(&mut self) -> String {
        match self.channel_type {
            ChannelType::Channel => { format!("https://www.youtube.com/feeds/videos.xml?channel_id={}", self.channel_id) },
            ChannelType::User => { format!("https://www.youtube.com/feeds/videos.xml?user={}", self.channel_id) },
            ChannelType::C => { let _ = self.get_true_channel(); format!("https://www.youtube.com/feeds/videos.xml?channel_id={}", self.channel_id) }
        }
    } // end get_req_url */

    // Get the latest video ID
    pub fn get_vid_id_from_index(&self, index: u8) -> Result<String, ()> {
        // Make the command, execute it and get the stdout
        let out = Command::new("youtube-dl").arg("--skip-download").arg("--playlist-item").arg(format!("{}", index+1)).arg("--dump-json").arg(self.get_feed_url()).output().unwrap();
        let out_str = str::from_utf8(&out.stdout).unwrap();

        // Verify the JSON can be parsed
        if let Ok(parsed_out) = json::parse(out_str) {
            // Get the latest ID
            if let Some(latest_vid_id) = parsed_out["id"].as_str() {
                // Video ID's good - return it
                Ok(String::from(latest_vid_id))
            } else { Err(()) }
        } else { Err(()) }
    } // end get_latest_id

    // Get the true channel ID from a C type channel, returning the latest video ID
    fn get_true_channel(&mut self) -> Result<(), ()> {
        // Make the command, execute it and get the stdout
        let out = Command::new("youtube-dl").arg("--skip-download").arg("--playlist-end").arg("1").arg("--dump-json").arg(self.get_feed_url()).output().unwrap();
        let out_str = str::from_utf8(&out.stdout).unwrap();

        // Verify the JSON can be parsed
        if let Ok(parsed_out) = json::parse(out_str) {
            // Try to get the channel url as a string
            if let Some(found_ch_id) = parsed_out["channel_id"].as_str() {
                // Now we have the channel's URL as a string
                self.channel_id = String::from(found_ch_id);
                self.channel_type = ChannelType::Channel;
                Ok(())

            // Couldn't find the channel url
            } else { Err(()) }

        // If the data's not good, return an error
        } else { Err(()) }

    } // end get_true_channel

    pub fn write_channel_to_file(&self) -> Result<(), ()> {
        let json_string = serde_json::to_string(&self).unwrap();
        let mut file_to_save_to = std::fs::File::create(self.path.as_path()).unwrap();

        if let Err(_) = file_to_save_to.write_all(&json_string.into_bytes()) { Err(()) }
        else { Ok(()) }
    }

    pub fn update_id(&self, id: (Option<String>, Option<String>)) {
        println! ("updating id from {:?} to {:?}", self.latest_ids, id);
        let mut updated_ch = self.clone();
        updated_ch.latest_ids = id;
        updated_ch.write_channel_to_file().unwrap();
    }

    pub fn get_latest_id(&self, default: &String) -> (String, String) {
        if let (Some(id_1), Some(id_2)) = &self.latest_ids { (String::from(id_1), String::from(id_2)) }
        else { (default.to_string(), default.to_string()) }
    }
}

pub fn populate_video_from_id(id: &String) -> Result<Video, ()> {
    // Make the command, execute it and get the stdout
    let out = Command::new("youtube-dl").arg("--dump-json").arg(format!("https://www.youtube.com/watch?v={}", id)).output().unwrap();
    let out_str = str::from_utf8(&out.stdout).unwrap();

    // Parse the json
    if let Ok(parsed_out) = json::parse(out_str) {
        let title = if let Some(parsed_title) = parsed_out["title"].as_str() { parsed_title } else { return Err(()) };
        let desc = if let Some(parsed_desc) = parsed_out["description"].as_str() { parsed_desc } else { return Err(()) };
        let live = if let Some(parsed_live) = parsed_out["is_live"].as_bool() { parsed_live } else { false };
        let tags = {
            let tags_raw = String::from(parsed_out["tags"].dump());
            let mut chars = tags_raw.chars();
            chars.next();
            chars.next();
            chars.next_back();
            chars.next_back();
            let bracketless_tags = String::from(chars.as_str());
            let split = bracketless_tags.split("\",\"");
            split.map(|st| String::from(st)).collect::<Vec<String>>()
        };

        Ok(Video {
            video_title: String::from(title),
            video_id: String::from(id),
            video_desc: String::from(desc),
            is_live: live,
            tags: Some(tags)
        })
    } else { Err(()) }
}

/*
fn populate_video_from_id(lines: &String) -> Video {
    // Get the title
    let titles_regex = regex::Regex::new("<title>|</title>").unwrap();
    let title_vec = titles_regex.split(lines.as_str()).collect::<Vec<_>>();
    let title = String::from(*title_vec.get(1).unwrap());

    // Get the ID
    let id_regex = regex::Regex::new("<yt:videoId>|</yt:videoId>").unwrap();
    let id_vec = id_regex.split(lines.as_str()).collect::<Vec<_>>();
    let id = String::from(*id_vec.get(1).unwrap());

    // Get the desc
    let desc_regex = regex::Regex::new("<media:description>|</media:description>").unwrap();
    let desc_vec = desc_regex.split(lines.as_str()).collect::<Vec<_>>();
    let desc = String::from(*desc_vec.get(1).unwrap());

    Video {
        video_title: title,
        video_id: id,
        video_desc: desc,
        image: None
    }
}*/

/*fn get_page_lines(url: &String) -> String {
    // Request it
    let res = easy_http_request::DefaultHttpRequest::get_from_url_str(url).unwrap().send().unwrap();
    String::from_utf8(res.body).unwrap()
}*/
