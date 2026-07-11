# Custom Transition Shaders Guide

Liem Wallpaper allows you to write, add, and use your own GPU-accelerated transition effects by writing standard High-Level Shader Language (HLSL) pixel shaders.

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



## Best Practices & Tips

1.  **Aspect Ratio**: Widescreen monitors distort standard UV coordinates ($0.0 \to 1.0$). Always multiply `(uv.x - 0.5)` by `(width / height)` if you need symmetrical circular or radial math.
2.  **Performance**: Avoid complex loops or branches (`for`, `while`) inside the pixel shader. The GPU compiles branch code, but keeping it math-based (`step`, `smoothstep`, `lerp`, `abs`) ensures high performance.
3.  **Feathering**: Use `smoothstep` on distance boundaries instead of harsh `if/else` checks. This creates clean, soft-feathered edges and eliminates jagged pixel artifacts (aliasing).
