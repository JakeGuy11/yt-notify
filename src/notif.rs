extern crate notify_rust;
use crate::youtube::{Video, Channel};
use notify_rust::Notification;

#[derive(Debug)]
pub enum ExecError {
    EmptyVideo,
    EmptyChannel,
    EmptyPreferences
}

#[derive(Debug, Clone)]
pub struct NotifPrefs {
    timeout: Option<u8>,
    urgency: Option<notify_rust::NotificationUrgency>
}

#[derive(Debug, Clone)]
pub struct Notif<'a> {
    video_field: Option<&'a Video>,
    channel_field: Option<&'a Channel>,
    prefs_field: Option<&'a NotifPrefs>
}

impl<'a> Notif<'a> {
    pub fn new() -> Notif<'a> {
        Notif {
            video_field: None,
            channel_field: None,
            prefs_field: None
        }
    }

    pub fn video(&mut self, vid: &'a Video) -> &mut Notif<'a> {
        self.video_field = Some(vid);
        self
    }

    pub fn channel(&mut self, chan: &'a Channel) -> &mut Notif<'a> {
        self.channel_field = Some(chan);
        self
    }

    pub fn preferences(&mut self, pref: &'a NotifPrefs) -> &mut Notif<'a> {
        self.prefs_field = Some(pref);
        self
    }

    pub fn build(&self) -> Result<(), ExecError> {
        self.verify_validity()?;
        Ok(())
    }

    pub fn exec(&self) -> Result<(), notify_rust::Error>{
        let channel = self.channel_field.unwrap();
        let video = self.video_field.unwrap();
        let prefs = self.prefs_field.unwrap();

        if passes_notif_filter(video, channel) {
            let summary = video.video_title.as_str();
            let body = if video.is_live { format! ("{} is live", channel.name) } else { format! ("{} has uploaded a video", channel.name) };
            let icon = std::path::PathBuf::from(&channel.pic_path);
            let timeout = notify_rust::Timeout::Milliseconds(prefs.timeout.unwrap() as u32 * 1000);
            let urgency = prefs.urgency.unwrap();

            Notification::new()
                .summary(summary)
                .body(body.as_str())
                .icon(icon.to_str().unwrap())
                .timeout(timeout)
                .urgency(urgency)
                .show()?;
            
            Ok(())
        } else { Ok(())}
    }

    fn verify_validity(&self) -> Result<(), ExecError> {
        if let None = self.video_field { Err(ExecError::EmptyVideo) }
        else if let None = self.channel_field { Err(ExecError::EmptyChannel) }
        else if let None = self.prefs_field { Err(ExecError::EmptyPreferences) }
        else { Ok(()) }
    }
}

impl NotifPrefs {
    pub fn new() -> NotifPrefs {
        NotifPrefs {
            timeout: None,
            urgency: None
        }
    }

    pub fn timeout(&mut self, time: u8) -> &mut NotifPrefs {
        self.timeout = Some(time);
        self
    }

    pub fn urgency(&mut self, urg: notify_rust::NotificationUrgency) -> &mut NotifPrefs {
        self.urgency = Some(urg);
        self
    }
}

fn passes_notif_filter(video: &Video, channel: &Channel) -> bool {
    true
}