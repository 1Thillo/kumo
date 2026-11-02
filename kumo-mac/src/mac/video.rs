//! Video wallpapers through AVFoundation. The Mac decodes in hardware and composes the frames
//! itself, so a video needs neither the render loop nor a texture of its own, and screens that
//! show the same video share one player.

use crate::config::Fit;
use crate::media::ns_url;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_av_foundation::{
    AVLayerVideoGravityResize, AVLayerVideoGravityResizeAspect, AVLayerVideoGravityResizeAspectFill, AVPlayerItem, AVPlayerItemStatus, AVPlayerLayer, AVPlayerLooper, AVPlayerLooperStatus, AVQueuePlayer, AVURLAsset,
};
use std::path::Path;

pub struct Video {
    player: Retained<AVQueuePlayer>,
    looper: Retained<AVPlayerLooper>,
    speed: f32,
    playing: bool,
}

impl Video {
    pub fn new(path: &Path, speed: f32, mtm: MainThreadMarker) -> Video {
        unsafe {
            let asset = AVURLAsset::URLAssetWithURL_options(&ns_url(path), None);
            let item = AVPlayerItem::playerItemWithAsset(&asset, mtm);
            let player = AVQueuePlayer::new(mtm);
            player.setMuted(true);
            // a wallpaper must never keep the Mac awake
            player.setPreventsDisplaySleepDuringVideoPlayback(false);
            let looper = AVPlayerLooper::playerLooperWithPlayer_templateItem(&player, &item);
            Video { player, looper, speed, playing: false }
        }
    }

    /// A layer for one screen; it stays tied to this player.
    pub fn layer(&self, fit: Fit) -> Retained<AVPlayerLayer> {
        unsafe {
            let layer = AVPlayerLayer::playerLayerWithPlayer(Some(&self.player));
            let gravity = match fit {
                Fit::Cover => AVLayerVideoGravityResizeAspectFill,
                Fit::Contain => AVLayerVideoGravityResizeAspect,
                Fit::Stretch => AVLayerVideoGravityResize,
            };
            if let Some(gravity) = gravity {
                layer.setVideoGravity(gravity);
            }
            layer
        }
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
        if self.playing {
            unsafe { self.player.setRate(speed) };
        }
    }

    pub fn set_playing(&mut self, on: bool) {
        if on == self.playing {
            return;
        }
        self.playing = on;
        unsafe {
            if on {
                self.player.setRate(self.speed);
            } else {
                self.player.pause();
            }
        }
    }

    /// Why the video cannot play, once AVFoundation has looked at it (webm and mkv end here).
    pub fn error(&self) -> Option<String> {
        unsafe {
            if self.looper.status() == AVPlayerLooperStatus::Failed {
                return Some(self.looper.error().map_or_else(|| "this video format cannot be decoded".to_string(), |e| e.localizedDescription().to_string()));
            }
            let item = self.player.currentItem()?;
            (item.status() == AVPlayerItemStatus::Failed).then(|| item.error().map_or_else(|| "this video format cannot be decoded".to_string(), |e| e.localizedDescription().to_string()))
        }
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        unsafe {
            self.looper.disableLooping();
            self.player.pause();
            self.player.removeAllItems();
        }
    }
}
