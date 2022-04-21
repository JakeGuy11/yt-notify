extern crate home;
mod youtube;
use std::path::{PathBuf};
use std::io::Write;
use youtube::Channel;

#[derive(Debug)]
enum Intent {
    AddChannel(Result<youtube::Channel, ()>),
    RemoveChannel(String),
    EditChannel(String),
    StartDaemon,
    DumpEntries
}

fn main() {
    let cfg_path = verify_save_dirs();
    let mut usr_start_daemon = false;

    for current_intent in find_intents(&cfg_path).iter() {
        println! ("{:?}", current_intent);
        match current_intent {
            Intent::AddChannel(channel_opt) => {
                if let Ok(channel) = channel_opt {
                    if let Err(_) = youtube::Channel::write_channel_to_file(channel) {
                        eprintln! ("Could not write channel to file. Do you have permission?");
                    } else {
                        println! ("Added {} successfully.", channel.name);
                    }
                } else { eprintln! ("Could not verify that channel. Is the URL correct?"); }
            },
            Intent::RemoveChannel(id) => {
                println! ("Eventually, I will remove the channel with id {}", id);
            },
            Intent::EditChannel(id) => {
                println! ("Eventually, I will edit the channel with id {}", id);
            },
            Intent::StartDaemon => { usr_start_daemon = true; },
            Intent::DumpEntries => { 
                for ch_path in get_saved_entries(&cfg_path).iter() {
                    let channel = Channel::from_file(ch_path).unwrap();
                    println! ("{:?}", channel);
                }
            }
        }
    }

    if usr_start_daemon { println! ("Starting daemon..."); start_daemon(&cfg_path); }
}

fn start_daemon(cfg_path: &PathBuf) {
    let mut all_channels: Vec<Channel> = Vec::new();

    loop {
        println! ("In periodic");
        // Get all the entries
        // Populate all channels
        for ch_path in get_saved_entries(cfg_path).iter() {
            all_channels.push(Channel::from_file(ch_path).unwrap());
        }

        // Go through each channel
        for channel in all_channels.iter() {
            // found the last notified id?
            let mut found_last_id = false;
            let mut id_to_update: (Option<String>, bool) = (None, false);
            // Got through the 3 latest videos
            for i in 0..3 {
                // If we found it, keep going
                if found_last_id { break; }
                println! ("on iteration {}", i);

                // Get the video
                let latest_vid = channel.clone().get_vid_id_from_index(i).unwrap();
                if let Ok(that_vid) = youtube::populate_video_from_id(&latest_vid) {
                    // check if it's the latest id
                    println! ("{}, {}", that_vid.video_id, channel.get_latest_id(&that_vid.video_id));
                    if that_vid.video_id == channel.get_latest_id(&that_vid.video_id) {
                        id_to_update = (Some(String::from(&that_vid.video_id)), true);
                        found_last_id = true;
                    }

                    // if it's the first video and 
                    if i == 0 && !found_last_id {
                        id_to_update.0 = Some(String::from(&that_vid.video_id));
                        id_to_update.1 = true;
                        notify_video(&that_vid);
                    } else if !found_last_id {
                        notify_video(&that_vid);
                    }
                }
            }
            if id_to_update.1 { channel.update_id(id_to_update.0); }
        }

        // Wait for the next check
        all_channels = Vec::new();
        std::thread::sleep(std::time::Duration::from_secs(15));
    }
}

fn notify_video(vid: &youtube::Video) {
    println! ("{:?}", vid.video_title);
}

// Parse command line arguments
fn find_intents(save_path: &PathBuf) -> std::vec::Vec<Intent> {
    let all_args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut ret_intents = Vec::new();

    // Go through each command line arg
    for arg in 0..all_args.len() {
        //Check if it's an arg we're looking for
        match all_args.get(arg).unwrap().as_str() {
            "-s" | "--start-daemon" => { ret_intents.push(Intent::StartDaemon); },
            "-a" | "--add-channel" => { ret_intents.push(Intent::AddChannel(prompt_channel(save_path))); },
            "-r" | "--remove-channel" => { ret_intents.push(Intent::RemoveChannel(prompt_string("Enter the \x1b[93mID\x1b[0m of the channel you would like to remove:"))); },
            "-e" | "--edit-channel" => { ret_intents.push(Intent::EditChannel(prompt_string("Enter the \x1b[93mID\x1b[0m of the channel you would like to edit:"))); },
            "-d" | "--dump" => { ret_intents.push(Intent::DumpEntries); },
            _ => {  }
        }
    }

    if ret_intents.len() > 0 { ret_intents }
    else { vec! [Intent::StartDaemon] }
}

// Prompt the user for info about a channel, construct and return it
fn prompt_channel(save_path: &PathBuf) -> Result<youtube::Channel, ()> {
    // Get everything we need for the video
    let name = prompt_string("Enter the nickname of the channel you'd like to add:");
    let url = prompt_string("Enter the URL of the channel you'd like to add:");
    let keywords_str = prompt_string("Enter a comma-separated list of words you'd like to receive notifications for. Leave blank if you would like to receive everything.");
    let mut keywords_string: Vec<String> = Vec::new();
    for current_str in keywords_str.split(",") { keywords_string.push(String::from(current_str)); }

    println! ("Verifying and saving channel \"{}\"...", name);
    youtube::Channel::new(name, url, save_path, keywords_string)
}

// Prompt the user for a string
fn prompt_string(prompt: &str) -> String {
    println! ("{}", prompt);

    std::io::stdout().flush().unwrap();
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).unwrap();
    buf.truncate(buf.len()-1);
    buf
}

// Get the expected save directory (for windows)
#[cfg(target_os = "windows")]
fn get_save_dirs() -> (PathBuf, PathBuf) {
    let save_path_opt = home::home_dir();

    if let Some(mut cfg_path) = save_path_opt {
        cfg_path.push("AppData");
        cfg_path.push("Local");
        cfg_path.push("yt-notify");

        let mut pic_path = cfg_path.clone();
        pic_path.push("icons");

        (cfg_path, pic_path)
    } else {
        eprintln! ("FATAL: Could not find home directory!");
        std::process::exit(1);
    }
}

// Get the expected save directory (for linux/mac)
#[cfg(not(target_os = "windows"))]
fn get_save_dirs() -> (PathBuf, PathBuf) {
    let save_path_opt = home::home_dir();

    if let Some(mut cfg_path) = save_path_opt {
        cfg_path.push(".local");
        cfg_path.push("share");
        cfg_path.push("yt-notify");

        let mut pic_path = cfg_path.clone();
        pic_path.push("icons");

        (cfg_path, pic_path)
    } else {
        eprintln! ("FATAL: Could not find home directory!");
        std::process::exit(1);
    }
}

// Verify that the expected save directory is valid, return it
fn verify_save_dirs() -> PathBuf {
    let paths = get_save_dirs();
    let cfg_path = paths.0.clone();
    let pic_path = paths.1;

    let paths_to_make = vec![cfg_path, pic_path];

    for current_path in paths_to_make.iter() {
        // If the path already exists
        if current_path.exists() {
            // Something exists there - what is it?
            if !current_path.is_dir() {
                // Path is not a dir
                eprintln!("The expected config dir is a file! Please delte it at {} to continue.", current_path.clone().into_os_string().into_string().unwrap());
                std::process::exit(1);
            } else {
                // We're all good
                continue;
            }
        // The path doesn't exist
        } else {
            // Create it, return it
            if let Ok(()) = std::fs::create_dir_all(&current_path) {
                // It was made properly, go on
                continue;
            } else {
                eprintln!("Could not create config dir at {}! Do you have permission?", current_path.clone().into_os_string().into_string().unwrap());
                std::process::exit(1);
            }
        }
    }

    paths.0
}

// Get all the entries in the folder
fn get_saved_entries(entry_path: &PathBuf) -> Vec<PathBuf> {
    let all_paths = std::fs::read_dir(entry_path).unwrap();

    let mut ret_vec: Vec<PathBuf> = Vec::new();
    for current_path_res in all_paths {
        if let Ok(current_path) = current_path_res {
            if !current_path.path().is_dir() { ret_vec.push(current_path.path()); }
        } else { continue; }
    }
    
    ret_vec
}
