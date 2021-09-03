If you wanna know how to use mutable structured data across multiple threads and program instances in rust-lang; while keeping a thread-safe savefile backup. Check out the AOG project src-v2-rust directory on Github.

This allows the AOG software to run in the background listening for commands from other foreground instances of the software. This way pumps, lights, etc aren't being controlled from multiple instances haphazardly. 

TLDR: The background app does the real work and the foreground app is able to provide an interactive console.

-----------------------------------------------------------------------