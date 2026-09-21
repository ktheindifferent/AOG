WIP!!! Do not use!!

I highly recommend using the python version for now. It's messy but it works. 

The Rust version will have a smaller memory footprint, better memory safety, and more features in the long term. 

The hardware will always remain the same or similar. v2 will be reverse compatible with v1 A.O.G. builds.

## Building on Debian, Ubuntu, or Raspberry Pi OS

With Rust installed, run these commands from this directory:

```sh
sh ./setup.sh
cargo build --locked
```

The setup script installs the native build dependencies, including `libv4l-dev`.
The camera dependency needs its `libv4l2.so` linker file; having only the runtime
library (`libv4l2.so.0`) is not sufficient. If the build reports
`unable to find library -lv4l2`, install `libv4l-dev` and rebuild.
