# WebSockify-rs
[![Build](https://github.com/informationsea/websockify-rs/actions/workflows/build.yml/badge.svg)](https://github.com/informationsea/websockify-rs/actions/workflows/build.yml)

Rust implementation of [WebSockify](https://github.com/novnc/websockify).
noVNC files are embedded in websockify-rs for easy use.

## Usage

```
WebSockify-rs 0.1.0
Okamura Yasunobu
Convert TCP/Unix doamin socket connection to WebSocket

USAGE:
    websockify-rs [FLAGS] [OPTIONS] <upstream> <listen>

FLAGS:
    -h, --help             Prints help information
    -l, --listen-unix     Listen path is unix domain socket
    -u, --upstream-unix    Upstream is unix domain socket
    -V, --version          Prints version information
    -v, --verbose          

OPTIONS:
    -p, --prefix <prefix>    server prefix
        --novnc <version>    Embedded noVNC version to serve [default: 1.7.0]

ARGS:
    <upstream>    Upstream host:port
    <listen>      Listen host:port
```

## Example

```
$ vncserver
$ ./websockify-rs localhost:5901 127.0.0.1:8080
$ ./websockify-rs --novnc 1.6.0 localhost:5901 127.0.0.1:8080
```

Access to [http://127.0.0.1/static/vnc.html](http://127.0.0.1/static/vnc.html)

### Legacy VNC compatibility (opt-in)

Vendored noVNC 1.3.0, 1.6.0, and 1.7.0 support a per-connection workaround for RFB 3.3 peers named `rfb` that advertise little-endian RGB565. Add `?native_raw=true` to `vnc.html` or `vnc_lite.html` for the affected connection only. The setting is off by default and is not saved in the browser. JavaScript integrations can pass `{ nativeRaw: true }` in the `RFB` constructor options.

When enabled for a matching peer, noVNC skips both `SetPixelFormat` and `SetEncodings`. It uses uncompressed Raw, so quality and compression settings do not apply. Other peers keep their existing negotiation behavior even if the option is set.

To run the browser regression, serve the repository root with `python3 -m http.server 8000 --bind 127.0.0.1` and open [tests/novnc-native-raw.html](http://127.0.0.1:8000/tests/novnc-native-raw.html). It uses a scripted peer, not a live VNC server.