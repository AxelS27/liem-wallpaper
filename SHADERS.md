# Custom Transition Shaders Guide

Liem Wallpaper allows you to write, add, and use your own GPU-accelerated transition effects by writing standard High-Level Shader Language (HLSL) pixel shaders.

---

## Where Shaders Are Loaded From

When a transition is requested, Liem Wallpaper compiles and runs the corresponding `.hlsl` shader file. The service searches for shaders in:
1.  **Local Folder**: The `shaders/` directory next to `lw-service.exe` in your installation folder.
2.  **Global Folder**: `%APPDATA%\LiemWallpaper\shaders\`

To add a new transition, simply drop a `.hlsl` file (e.g. `wave.hlsl`) into one of these folders. You can immediately call it using the CLI:
```powershell
lw set "C:\wallpaper.jpg" --transition wave
```

---

## HLSL Environment & Input Variables

Your shader runs as a **D3D11 Pixel Shader (Target `ps_5_0`)**. It receives the current wallpaper texture, the new wallpaper texture, a bilinear sampler, and a constant buffer containing variables updated every frame.

### Shader Template

Use this starter template for every custom transition shader:

```hlsl
// t0 contains the old/current wallpaper
Texture2D TextureFrom : register(t0);

// t1 contains the new wallpaper
Texture2D TextureTo : register(t1);

// s0 is the standard bilinear sampler state
SamplerState Sampler : register(s0);

// Constant Buffer containing transition metadata (automatically updated per-frame)
cbuffer TransitionParams : register(b0) {
    float progress;  // The progress of the transition, scaled 0.0 to 1.0 (eased)
    float width;     // The active monitor width in pixels
    float height;    // The active monitor height in pixels
    float duration;  // The total transition duration in milliseconds
};

struct PS_INPUT {
    float4 position : SV_POSITION;
    float2 uv : TEXCOORD0; // Texture coordinates, mapped 0.0 to 1.0
};

float4 main(PS_INPUT input) : SV_TARGET {
    // ----------------------------------------------------
    // Your custom transition logic goes here!
    // Must return a float4 (RGBA color value).
    // ----------------------------------------------------
    
    // Default fallback: simple fade
    float4 colorFrom = TextureFrom.Sample(Sampler, input.uv);
    float4 colorTo = TextureTo.Sample(Sampler, input.uv);
    return lerp(colorFrom, colorTo, progress);
}
```

---

## Direct3D 11 Shader Examples

Here are some real examples of transitions that you can copy, modify, and experiment with:

### Example 1: Horizontal Slide (Wipe)
Slides the new wallpaper from right to left over the old one:

```hlsl
Texture2D TextureFrom : register(t0);
Texture2D TextureTo : register(t1);
SamplerState Sampler : register(s0);

cbuffer TransitionParams : register(b0) {
    float progress;
    float width;
    float height;
    float duration;
};

struct PS_INPUT {
    float4 position : SV_POSITION;
    float2 uv : TEXCOORD0;
};

float4 main(PS_INPUT input) : SV_TARGET {
    // Slide left: incoming wallpaper slides in from the right side
    if (input.uv.x > (1.0 - progress)) {
        // Sample incoming wallpaper offset by the progress
        float2 incoming_uv = float2(input.uv.x - (1.0 - progress), input.uv.y);
        return TextureTo.Sample(Sampler, incoming_uv);
    } else {
        // Sample old wallpaper offset by the progress
        float2 old_uv = float2(input.uv.x + progress, input.uv.y);
        return TextureFrom.Sample(Sampler, old_uv);
    }
}
```

### Example 2: Retro Pixelation
Pixelates the screen, increasing pixel size towards the middle of the transition (0.5 progress), then sharpening back into the new wallpaper:

```hlsl
Texture2D TextureFrom : register(t0);
Texture2D TextureTo : register(t1);
SamplerState Sampler : register(s0);

cbuffer TransitionParams : register(b0) {
    float progress;
    float width;
    float height;
    float duration;
};

struct PS_INPUT {
    float4 position : SV_POSITION;
    float2 uv : TEXCOORD0;
};

float4 main(PS_INPUT input) : SV_TARGET {
    // Calculate pixel size: peak pixelation at 0.5 progress
    float strength = 1.0 - abs(progress - 0.5) * 2.0;
    
    // Scale pixel size between 1 (no pixelation) and 100 pixels
    float pixelSize = 1.0 + strength * 99.0;
    
    // Map UV coordinates to coarse pixels
    float2 pixelUV = float2(
        floor(input.uv.x * width / pixelSize) * pixelSize / width,
        floor(input.uv.y * height / pixelSize) * pixelSize / height
    );
    
    float4 colorFrom = TextureFrom.Sample(Sampler, pixelUV);
    float4 colorTo = TextureTo.Sample(Sampler, pixelUV);
    
    // Mix the two pixelated textures
    return lerp(colorFrom, colorTo, progress);
}
```

### Example 3: Aspect-Ratio Corrected Radial Wipe
A clock-like circular wipe that expands from the center of the screen, correcting for widescreen distortion:

```hlsl
Texture2D TextureFrom : register(t0);
Texture2D TextureTo : register(t1);
SamplerState Sampler : register(s0);

cbuffer TransitionParams : register(b0) {
    float progress;
    float width;
    float height;
    float duration;
};

struct PS_INPUT {
    float4 position : SV_POSITION;
    float2 uv : TEXCOORD0;
};

float4 main(PS_INPUT input) : SV_TARGET {
    // Correct aspect ratio so circular shape remains a perfect circle
    float aspectRatio = width / height;
    float2 center = float2(0.5, 0.5);
    
    // Scale X coordinate relative to center to correct distortion
    float2 uvCorrected = input.uv;
    uvCorrected.x = (uvCorrected.x - center.x) * aspectRatio + center.x;
    
    // Calculate distance from center
    float dist = distance(uvCorrected, center);
    
    // Maximum circular radius to cover the screen corners (approx. 0.8)
    float maxRadius = 0.8;
    float currentRadius = progress * maxRadius;
    
    // Smooth feathering boundary
    float feather = 0.05;
    float factor = smoothstep(currentRadius - feather, currentRadius + feather, dist);
    
    float4 colorFrom = TextureFrom.Sample(Sampler, input.uv);
    float4 colorTo = TextureTo.Sample(Sampler, input.uv);
    
    // Blend: new wallpaper inside the circle, old wallpaper outside
    return lerp(colorTo, colorFrom, factor);
}
```

---

## Best Practices & Tips

1.  **Aspect Ratio**: Widescreen monitors distort standard UV coordinates ($0.0 \to 1.0$). Always multiply `(uv.x - 0.5)` by `(width / height)` if you need symmetrical circular or radial math.
2.  **Performance**: Avoid complex loops or branches (`for`, `while`) inside the pixel shader. The GPU compiles branch code, but keeping it math-based (`step`, `smoothstep`, `lerp`, `abs`) ensures high performance.
3.  **Feathering**: Use `smoothstep` on distance boundaries instead of harsh `if/else` checks. This creates clean, soft-feathered edges and eliminates jagged pixel artifacts (aliasing).
