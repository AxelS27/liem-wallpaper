Texture2D TextureFrom : register(t0);
Texture2D TextureTo : register(t1);
SamplerState Sampler : register(s0);

cbuffer TransitionParams : register(b0) {
    float progress;
    float3 padding;
};

struct PS_INPUT {
    float4 position : SV_POSITION;
    float2 uv : TEXCOORD0;
};

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
    // Correct for aspect ratio using screen space derivatives
    float aspect = ddy(input.uv.y) / ddx(input.uv.x);

    if (progress <= 0.0) {
        float2 uvFrom = get_fill_uv(input.uv, TextureFrom, aspect);
        return TextureFrom.Sample(Sampler, uvFrom);
    }
    if (progress >= 1.0) {
        float2 uvTo = get_fill_uv(input.uv, TextureTo, aspect);
        return TextureTo.Sample(Sampler, uvTo);
    }

    float2 center = float2(0.5, 0.5);
    float2 d = input.uv - center;
    d.x *= aspect;
    
    float angle = atan2(d.x, -d.y); // Start at 12 o'clock, clockwise
    if (angle < 0.0) {
        angle += 2.0 * 3.14159265;
    }
    
    float norm_angle = angle / (2.0 * 3.14159265);
    
    // Smooth feathering at the sweep edge
    float feather = 0.005;
    float factor = smoothstep(progress - feather, progress + feather, norm_angle);
    
    float2 uvFrom = get_fill_uv(input.uv, TextureFrom, aspect);
    float2 uvTo = get_fill_uv(input.uv, TextureTo, aspect);

    float4 colorFrom = TextureFrom.Sample(Sampler, uvFrom);
    float4 colorTo = TextureTo.Sample(Sampler, uvTo);
    
    return lerp(colorTo, colorFrom, factor);
}
