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
// Must be aligned to 16 bytes.
cbuffer TransitionParams : register(b0) {
    float progress;  // The progress of the transition, scaled 0.0 to 1.0 (eased)
    float3 padding;  // 12-byte padding for 16-byte D3D11 constant buffer alignment
};

struct PS_INPUT {
    float4 position : SV_POSITION;
    float2 uv : TEXCOORD0; // Texture coordinates, mapped 0.0 to 1.0
};

// Aspect ratio correction helper to crop/fill the texture like Windows native "Fill" mode
float2 get_fill_uv(float2 uv, Texture2D tex, float aspect) {
    uint tw, th;
    tex.GetDimensions(tw, th);
    float tex_aspect = (float)tw / (float)th;
    
    float2 new_uv = uv;
    if (tex_aspect > aspect) {
        float scale_u = aspect / tex_aspect;
        new_uv.x = uv.x * scale_u + 0.5 * (1.0 - scale_u);
    } else {
        float scale_v = tex_aspect / aspect;
        new_uv.y = uv.y * scale_v + 0.5 * (1.0 - scale_v);
    }
    return new_uv;
}

float4 main(PS_INPUT input) : SV_TARGET {
    // 1. Calculate screen/monitor aspect ratio dynamically using screen space derivatives
    float aspect = ddy(input.uv.y) / ddx(input.uv.x);

    // 2. Early-exit guards to prevent edge feathering/sweep line artifacts
    if (progress <= 0.0) {
        return TextureFrom.Sample(Sampler, get_fill_uv(input.uv, TextureFrom, aspect));
    }
    if (progress >= 1.0) {
        return TextureTo.Sample(Sampler, get_fill_uv(input.uv, TextureTo, aspect));
    }

    // 3. Sample colors using Fill aspect ratio scaling
    float2 uvFrom = get_fill_uv(input.uv, TextureFrom, aspect);
    float2 uvTo = get_fill_uv(input.uv, TextureTo, aspect);
    float4 colorFrom = TextureFrom.Sample(Sampler, uvFrom);
    float4 colorTo = TextureTo.Sample(Sampler, uvTo);
    
    // Default fallback: simple fade transition
    return lerp(colorFrom, colorTo, progress);
}
```

## Best Practices & Tips

1.  **Aspect Ratio Correction**: Widescreen monitors distort standard UV coordinates ($0.0 \to 1.0$). Calculate `float aspect = ddy(uv.y) / ddx(uv.x)` dynamically in your pixel shader and multiply your horizontal offsets by this aspect ratio.
2.  **Fill Mode Scaling**: Use the `get_fill_uv` helper in the template above. This ensures that any wallpaper image maintains its correct aspect ratio without stretching, cropping exactly like the Windows native `Fill` setting.
3.  **Feathering**: Use `smoothstep` on distance boundaries instead of harsh `if/else` checks. This creates clean, soft-feathered edges and eliminates jagged pixel artifacts (aliasing).
4.  **Early Exit Guards**: Always add `progress <= 0.0` and `progress >= 1.0` checks at the very start of the main shader function. This completely eliminates thin lines or boundary sweep artifacts when the transition starts or completes.
