# Bachelor-Projekt

## Build

### Nix

```
nix build
```

### Cargo

Install the following dependencies:
```
openssl
wayland
wayland-protocols
vulkan-loader
vulkan-headers
libGL
```

Build with:
```
cargo build -p burp --release
```

The binaries will be placed in `target/release`
