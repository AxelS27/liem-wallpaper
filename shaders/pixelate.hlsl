Texture2D textureFrom : register(t0);
Texture2D textureTo : register(t1);
SamplerState samplerState : register(s0);

cbuffer TransitionParams : register(b0) {
    float progress;
    float3 padding;
};

struct PS_INPUT {
    float4 pos : SV_POSITION;
    float2 tex : TEXCOORD0;
};

float4 main(PS_INPUT input) : SV_Target {
    if (progress <= 0.0) {
        return textureFrom.Sample(samplerState, input.tex);
    }
    if (progress >= 1.0) {
        return textureTo.Sample(samplerState, input.tex);
    }

    float d = abs(progress - 0.5) * 2.0; // goes 1.0 (start) -> 0.0 (mid) -> 1.0 (end)
    float pixels = 8.0 + pow(d, 4.0) * 1024.0;
    float2 pixelatedTex = floor(input.tex * pixels) / pixels;

    if (progress < 0.5) {
        return textureFrom.Sample(samplerState, pixelatedTex);
    } else {
        return textureTo.Sample(samplerState, pixelatedTex);
    }
}
