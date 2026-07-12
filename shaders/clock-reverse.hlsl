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

float4 main(PS_INPUT input) : SV_TARGET {
    float2 center = float2(0.5, 0.5);
    float2 d = input.uv - center;
    
    // Correct for aspect ratio using screen space derivatives
    float aspect = ddy(input.uv.y) / ddx(input.uv.x);
    d.x *= aspect;
    
    float angle = atan2(-d.x, -d.y); // Start at 12 o'clock, counter-clockwise
    if (angle < 0.0) {
        angle += 2.0 * 3.14159265;
    }
    
    float norm_angle = angle / (2.0 * 3.14159265);
    
    // Smooth feathering at the sweep edge
    float feather = 0.005;
    float factor = smoothstep(progress - feather, progress + feather, norm_angle);
    
    float4 colorFrom = TextureFrom.Sample(Sampler, input.uv);
    float4 colorTo = TextureTo.Sample(Sampler, input.uv);
    
    return lerp(colorTo, colorFrom, factor);
}
