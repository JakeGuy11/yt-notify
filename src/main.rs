extern crate home;
extern crate tokio;
mod youtube;
mod notif;
use std::path::{PathBuf};
use std::io::Write;
use youtube::{Channel, Video};
use notif::{Notif, NotifPrefs};

#[derive(Debug)]
enum Intent {
    AddChannel(Result<youtube::Channel, ()>),
    RemoveChannel(String),
    EditChannel(String),
    Archive,
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
            Intent::Archive => {
                let p = std::path::Path::new("/home/jake/downloads/").to_path_buf();
                let rt = tokio::runtime::Runtime::new().unwrap();
                let start_fn = start_archive_daemon(&cfg_path, &p);
                rt.block_on(start_fn);
                
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
    // Update all channels first
    for ch_path in get_saved_entries(cfg_path).iter() {
        if let Ok(ch) = Channel::from_file(ch_path) {
            if let Err(_) = ch.init_update() {
                eprintln! ("Could not re-initialize channel {}; using latest ids {} and {}",
                    ch.name,
                    ch.get_latest_id(&"INVALID".to_string()).0,
                    ch.get_latest_id(&"INVALID".to_string()).1);
            }
        }
    }

    // Start the loop
    loop {
        // Populate all channels
        let mut all_channels: Vec<Channel> = Vec::new();
        for ch_path in get_saved_entries(cfg_path).iter() {
            if let Ok(ch) = Channel::from_file(ch_path) { all_channels.push(ch); }
        }

        // Go through each channel
        for channel in all_channels.iter() {
            // found the last notified id?
            let mut found_last_id = false;
            // Got through the 3 latest videos
            for i in 0..3 {
                // If we found it, keep going
                if found_last_id { break; }

                // Get the video
                if let Ok(latest_vid) = channel.clone().get_vid_id_from_index(i) {
                    if let Ok(that_vid) = youtube::populate_video_from_id(&latest_vid) {
                        // check if it's the latest id
                        let latest_ids = channel.get_latest_id(&that_vid.video_id);
                        found_last_id = found_last_id || that_vid.video_id == latest_ids.0 || that_vid.video_id == latest_ids.1;

                        // if it's the first video we're checking and it's new, update it and notify the user respectively
                        if !found_last_id {
                            println! ("there's something new (current video id is {}; latest ids are {:?})", that_vid.video_id, channel.get_latest_id(&that_vid.video_id));
                            println! ("on iteration {}; found_last_id is now {}", i, found_last_id);
                            println! ("video id is {}", that_vid.video_id);
                            notify_video(&that_vid, &channel);
                            if i == 0 {
                                // Get the first 2 ids
                                let id_1 = String::from(&that_vid.video_id);
                                let id_2 = {
                                    if let Ok(second_vid_raw) = channel.clone().get_vid_id_from_index(1) {
                                        if let Ok(second_vid) = youtube::populate_video_from_id(&second_vid_raw) {
                                            Some(second_vid.video_id)
                                        } else { None }
                                    } else { None }
                                };
                                channel.update_id((Some(id_1), id_2));
                            }
                        }
                    }
                }
            }
        }

        // Wait for the next check
        std::thread::sleep(std::time::Duration::from_secs(15));
    }
}

async fn start_archive_daemon(cfg_path: &PathBuf, archive_path: &PathBuf) {
	loop {
		// Populate all channels
        let mut all_channels: Vec<Channel> = Vec::new();
        for ch_path in get_saved_entries(cfg_path).iter() {
            if let Ok(ch) = Channel::from_file(ch_path) {
				if ch.archive { all_channels.push(ch); }
			}
        }
		
		// Go through each channel
        for channel in all_channels.iter() {
            // Get the latest video
			let latest_vid = {
				if let Ok(id) = channel.get_vid_id_from_index(0) {
					if let Ok(vid) = youtube::populate_video_from_id(&id) { vid }
					else { continue; }
				} else { continue; }
			};
			
			// Continue if it's not live; if it is, check if we're archiving
			if !latest_vid.is_live { continue; }
			else {
				// Get what the title will be
				let date = "";
				let title = format! ("[{}]{}.mp4", date, latest_vid.video_title);
				let mut expected_path = archive_path.clone();
				expected_path.push(title);
				
				if !expected_path.as_path().exists() {
					// It's not being archived; start it
                    let _ = tokio::task::spawn_blocking(move || {
                        archive_stream(latest_vid.video_id,
                            String::from(expected_path.as_path().to_str().unwrap()))
                    });
				}
			}
        }

        // Wait for the next check
        std::thread::sleep(std::time::Duration::from_secs(15));
	}
}

fn archive_stream(vid_id: String, out_file: String) {
    let youtube_dl_output = std::process::Command::new("youtube-dl")
        .arg("-f")
        .arg("best")
        .arg("-g")
        .arg(format! ("https://www.youtube.com/watch?v={}", vid_id)).output();

    if let Ok(out_bad) = youtube_dl_output {
        if let Ok(out) = std::str::from_utf8(&out_bad.stdout) {
            // We have our download url; archive it
            let mut cmd = std::process::Command::new("ffmpeg");
            let _ = cmd.arg("-i")
                .arg(out)
                .arg("-loglevel")
                .arg("panic")
                .arg("-c")
                .arg("copy")
                .arg(out_file)
                .status();
        }
    }
}

fn notify_video(vid: &Video, channel: &Channel) {
    let mut prefs = NotifPrefs::new();
    prefs.timeout(0).urgency(notify_rust::NotificationUrgency::Normal);

    let mut notif = Notif::new();
    notif.video(vid).channel(channel).preferences(&prefs);

    if let Err(e) = notif.build() {
        eprintln! ("Couldn't notify; {:?}", e);
    } else {
        if let Err(e) = notif.exec() {
            eprintln! ("Couldn't notify; {:?}", e);
        }
    }
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
            "--archive" => { ret_intents.push(Intent::Archive); },
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

    // Archive settings
    let archive = match prompt_string("Would you like to archive livestreams from this channel? [Y/n]").as_str() {
        "y" | "Y" => true,
        _ => false
    };
    let a_filters = if archive {
        let a_keywords_str = prompt_string("Enter a comma-separated list of keywords you'd like to archive streams with. Leave blank if you would like to archive everything.");
        let mut a_keywords_string: Vec<String> = Vec::new();
        for current_str in a_keywords_str.split(",") { a_keywords_string.push(String::from(current_str)); }
        Some(a_keywords_string)
    } else { None };

    println! ("Verifying and saving channel \"{}\"...", name);
    Channel::new(name, url, save_path, keywords_string, archive, a_filters)
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
            if let Some(ext) = current_path.path().as_path().extension() {
                if ext.to_str() == Some("json") { ret_vec.push(current_path.path()); }
            }
        } else { continue; }
    }
    
    ret_vec
}
