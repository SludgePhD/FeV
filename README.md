# Ferrovanadium: VA-API bindings for Rust

This library provides high-level wrappers around **VA-API** (*libva*), the
Video Acceleration API used by Linux, Android, and BSD systems.

It loads *libva* at runtime, so there is no build-time dependency on it. This
matches the behavior typically expected of applications: if no hardware
acceleration is available, they should fall back to software decoding.

## Features

- Blatantly unsound API
- Completely unmaintained (developed only for my own personal amusement)
- Exposes almost none of the features VA-API has
