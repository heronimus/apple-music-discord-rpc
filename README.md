# apple-music-discord-rpc
Written in Rust with Objective-C Apple Framework binding -- without osascript polling.


### Under Development

TODO:
 - pause/resume/stopped handling (playbackstate)
 - Use Key-Value-Observing https://developer.apple.com/documentation/swift/using-key-value-observing-in-swift
 - loop vs while?
 - Classical music not registered (check for different mediaitem type)
 - tidy up `unsafe{}`
 - breakup files structure
 - cached track extras
 - fix timestamp
 - remove unused imported feature from cargo
