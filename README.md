# Liem Wallpaper

**Liem Wallpaper** is a modular, high-performance Windows desktop application designed to render smooth, GPU-accelerated wallpaper transitions.

---

## Key Features

1. **GPU-Accelerated Transitions**: Uses Direct3D 11 and DirectComposition to compile HLSL transition shaders dynamically, rendering smooth, tearing-free animations synced to V-Sync.
2. **Icon-Safe Windows Hooking**: Seamlessly injects composition visuals into the Win32 `WorkerW` window hierarchy, ensuring wallpaper transitions run behind desktop icons and shortcut layers without capturing user inputs.
3. **Smart Rotation Scheduler**: Automatically cycles desktop backgrounds at scheduled intervals. Uses GDI active window bounds check to detect fullscreen applications (e.g. games, presentation software) and defers rotations to avoid disruption.
4. **Local Named Pipe IPC**: Integrates background daemon, CLI control, and Slint settings UI via serialized JSON-over-IPC Windows Named Pipes.
5. **Zero-Resource Idle State**: Automatically releases WIC decoders, HLSL pipelines, swapchains, and visuals immediately post-transition. Idle resource footprint remains at **0.0% CPU** and **under 30MB RAM**.
6. **Multi-Monitor Configuration**: Auto-discovers active displays, maps screen coordinates, and instantiates independent DXGI swapchains to perform smooth transitions across all monitors in sync.

---

## Workspace Structure

The workspace is organized into modular crates:
- **[lw-core](crates/lw-core/)**: Common configuration, traits, logging setup, and error enums.
- **[lw-renderer](crates/lw-renderer/)**: Direct3D 11 device contexts, composition setup, window hooking, and WIC loaders.
- **[lw-transition](crates/lw-transition/)**: Interpolation curves, transition shaders compiler, and the core rendering engine.
- **[lw-wallpaper](crates/lw-wallpaper/)**: Native Windows COM wrapper for wallpaper manipulation and monitor coordinate discovery.
- **[lw-service](crates/lw-service/)**: Background daemon runner, IPC server, and scheduler task loop.
- **[lw-cli](crates/lw-cli/)**: Command-line control interface.
- **[lw-gui](crates/lw-gui/)**: Slint-based settings monitoring UI.
- **[lw-plugin](crates/lw-plugin/)**: Reserved slot for dynamic shaders loading.

---

## Quickstart

Verify the workspace builds and all tests pass:
```powershell
cargo check
cargo test
```

For end-to-end integration scenario guides, see **[Quickstart & Verification Guide](specs/001-wallpaper-transitions/quickstart.md)**.
For architectural design details, see **[Architecture Documentation](docs/architecture.md)**.
