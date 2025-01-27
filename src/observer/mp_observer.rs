use crate::handlers::update_discord_activity;

use discord_presence::Client;
use objc2::rc::Retained;
use objc2::{define_class, msg_send, sel, AllocAnyThread, DeclaredClass};
use reqwest::blocking::{Client as HttpClient, ClientBuilder};
use std::cell::RefCell;
use std::time::Duration;

use objc2_foundation::{ns_string, NSCopying, NSObject, NSObjectProtocol, NSString};
use objc2_foundation::{NSNotification, NSNotificationCenter};
use objc2_media_player::{
    MPMediaEntityPersistentID, MPMediaPlayback, MPMusicPlaybackState, MPMusicPlayerController,
};

pub struct MusicPlayerObserverIvars {
    object: Retained<MPMusicPlayerController>,
    playback_state_notification: Retained<NSString>,
    now_playing_item_notification: Retained<NSString>,
    http_client: HttpClient,
    discord_client: RefCell<Client>,
    previous_index: RefCell<MPMediaEntityPersistentID>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "MusicPlayerObserver"]
    #[ivars = MusicPlayerObserverIvars]
    pub struct MusicPlayerObserver;

    impl MusicPlayerObserver {
        #[unsafe(method(handlePlaybackStateChange:))]
        unsafe fn handle_playback_state_change(&self, notification: &NSNotification) {
            println!("<--->  Playback state changed");

            if let Some(object) = &notification.object() {
                // Cast the notification object to MPMusicPlayerController
                let player: &MPMusicPlayerController = msg_send![object, self];

                let previous_media_id = *self.ivars().previous_index.borrow();

                match player.nowPlayingItem() {
                    Some(item) => {
                        if item.persistentID() != previous_media_id {
                            println!(
                                "<---> MediaItemPersistentID changed from {} to {}",
                                previous_media_id,
                                item.persistentID()
                            );
                            // Store the new ID
                            *self.ivars().previous_index.borrow_mut() = item.persistentID();

                            //Update activity on changes
                            if let Err(e) = update_discord_activity(
                                player,
                                &mut self.ivars().discord_client.borrow_mut(),
                                &self.ivars().http_client,
                            ) {
                                eprintln!("DISCOR_RPC: error in discord_update_activity: {}", e);
                            }
                        }

                        println!(
                            "<--->    -- Playing Item Title: {:#?}",
                            item.title().unwrap()
                        );
                        println!("<--->    -- Playing Item ID: {:#?}", item.persistentID());
                        println!(
                            "<--->    -- Playing Item Duration: {:#?}",
                            item.playbackDuration()
                        );
                    }
                    None => {
                        println!("<--->    -- No Playing Item");
                    }
                }

                // Console Debug Section
                match player.playbackState() {
                    MPMusicPlaybackState::Playing => {
                        println!(
                            "<--->    -- player playbackState: playing {:#?}",
                            player.playbackState().0
                        );
                    }
                    MPMusicPlaybackState::Paused => {
                        println!(
                            "<--->    -- player playbackState: paused {:#?}",
                            player.playbackState().0
                        );
                    }
                    MPMusicPlaybackState::Stopped => {
                        println!(
                            "<--->    -- player playbackState: stopped {:#?}",
                            player.playbackState().0
                        );
                    }
                    _ => {
                        println!(
                            "<--->    -- player playbackState: unknown state {:#?}",
                            player.playbackState().0
                        );
                    }
                }
                println!(
                    "<--->    -- indexOfNowPlayingItem {:#?}",
                    player.indexOfNowPlayingItem()
                );
                println!(
                    "<--->    -- currentPlaybackTime: {:#?}",
                    player.currentPlaybackTime()
                );
            }
        }

        #[unsafe(method(handleNowPlayingItemChange:))]
        unsafe fn handle_now_playing_item_change(&self, notification: &NSNotification) {
            println!("<---> Now playing item / playlist changed");

            if let Some(object) = notification.object() {
                // Cast the notification object to MPMusicPlayerController
                let player: &MPMusicPlayerController = msg_send![&object, self];

                match player.nowPlayingItem() {
                    Some(item) => {
                        println!("<--->  Playing Item Title{:#?}", item.title().unwrap());
                    }
                    None => {
                        println!("<--->  No Playing Item");
                    }
                }
            }
        }
    }

    unsafe impl NSObjectProtocol for MusicPlayerObserver {}
);

impl MusicPlayerObserver {
    pub unsafe fn new() -> Retained<Self> {
        let observer = Self::alloc().set_ivars(MusicPlayerObserverIvars {
            object: MPMusicPlayerController::systemMusicPlayer(),
            playback_state_notification: ns_string!(
                "MPMusicPlayerControllerPlaybackStateDidChangeNotification"
            )
            .copy(),
            now_playing_item_notification: ns_string!(
                "MPMusicPlayerControllerNowPlayingItemDidChangeNotification"
            )
            .copy(),
            discord_client: RefCell::new(Client::new(773825528921849856)),
            http_client: ClientBuilder::new()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            previous_index: RefCell::new(MPMediaEntityPersistentID::from_be(0)),
        });
        let observer: Retained<Self> = unsafe { msg_send![super(observer), init] };

        observer
            .ivars()
            .discord_client
            .borrow()
            .on_ready(|_ctx| {
                println!("Discord RPC connected!");
            })
            .persist();

        observer.ivars().discord_client.borrow_mut().start();

        // Add notification observers
        unsafe {
            let notification_center = NSNotificationCenter::defaultCenter();

            // Add observer for playback state changes
            let _ = notification_center.addObserver_selector_name_object(
                &*observer,
                sel!(handlePlaybackStateChange:),
                Some(&observer.ivars().playback_state_notification),
                Some(&*observer.ivars().object),
            );

            // Add observer for now playing item changes
            let _ = notification_center.addObserver_selector_name_object(
                &*observer,
                sel!(handleNowPlayingItemChange:),
                Some(&observer.ivars().now_playing_item_notification),
                Some(&*observer.ivars().object),
            );

            // Start generating notifications
            observer
                .ivars()
                .object
                .beginGeneratingPlaybackNotifications();
        }

        observer
    }
}

impl Drop for MusicPlayerObserver {
    fn drop(&mut self) {
        unsafe {
            //Clear Discord Activity
            if let Err(e) = self.ivars().discord_client.borrow_mut().clear_activity() {
                eprintln!("DEBUG: error in clear_activity: {}", e);
            };
            println!("Disconnected from Discord RPC.");

            // Remove notification observers
            let notification_center = NSNotificationCenter::defaultCenter();
            notification_center.removeObserver_name_object(
                self,
                Some(&self.ivars().playback_state_notification),
                Some(&*self.ivars().object),
            );
            notification_center.removeObserver_name_object(
                self,
                Some(&self.ivars().now_playing_item_notification),
                Some(&self.ivars().object),
            );

            // Stop generating notifications
            self.ivars().object.endGeneratingPlaybackNotifications();
        }
    }
}
