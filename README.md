# Liem Wallpaper

**Liem Wallpaper** is a lightweight, high-performance, GPU-accelerated Windows desktop wallpaper manager. It runs silently in the background as a daemon and provides instant.

## Command Line Interface (CLI) Usage

You can control everything from any PowerShell or Command Prompt window using the `lw` tool:

### 1. View Current Status
Check if the daemon is active, see what wallpaper is currently displayed, and view scheduler info:
```powershell
lw status
```

### 2. Set Wallpaper Immediately
Change the desktop background to any image:
```powershell
lw set "C:\path\to\wallpaper.jpg"
```

You can customize the transition on the fly:
```powershell
# Set wallpaper with a pixelate transition over 2.5 seconds
lw set "C:\path\to\wallpaper.jpg" -t pixelate -d 2500

# Set wallpaper with a slide-left transition using ease-out-quint
lw set "C:\path\to\wallpaper.jpg" -t slide-left -d 1500 -s quint -g out
```

### 3. Trigger Next Wallpaper
If you have configured a wallpapers folder, trigger a transition to the next random wallpaper in the queue:
```powershell
lw next
```

### 4. Control the Scheduler
Enable or disable automatic wallpaper changes:
```powershell
# Start automated rotation
lw start

# Stop automated rotation
lw stop
```

---

## CLI Command Options

When setting a wallpaper or configuring defaults, you can customize the transition engine using these parameters:

### Transitions (`-t`, `--transition`)
Choose from the built-in GPU-accelerated HLSL transitions, or specify the name of any custom `.hlsl` shader file placed in your `shaders/` directory (see the [Custom Transition Shaders Guide](SHADERS.md) for how to build your own):
*   `fade`: Smooth fade.
*   `zoom-in`: concentric circular zoom scaling up.
*   `zoom-out`: concentric circular zoom scaling down.
*   `pixelate`: Retro pixelation effect.
*   `glitch`: Chromatic aberration glitch.
*   `radial-in` / `radial-out`: Circular clock wipe.
*   `slide-left` / `slide-right` / `slide-up` / `slide-down`: Sliding transition.

### Easing Styles (`-s`, `--style`)
*   `linear`, `sine`, `quad`, `cubic`, `quart`, `quint`, `exponential`, `circular`, `back`, `bounce`, `elastic`.

### Easing Directions (`-g`, `--dir`)
*   `in`, `out`, `inout`.



## Configuration (`config.toml`)

The application stores its configuration file at `config.toml` in your installation directory (next to `lw-service.exe`). You can edit it manually to set default values:

```toml
# The directory containing your desktop wallpapers
wallpapers_dir = "C:\\Users\\YourName\\Pictures\\Wallpapers"

[scheduler]
# Whether automated rotation is enabled on startup
enabled = true
# Rotation interval in minutes
interval_mins = 15
# Automatically launch the daemon service when Windows starts
run_on_startup = true

[transition_default]
# The default transition effect name
effect_type = "fade"
# The default duration in milliseconds
duration_ms = 1000
# The default easing curves
easing_style = "Quad"
easing_direction = "InOut"
```
