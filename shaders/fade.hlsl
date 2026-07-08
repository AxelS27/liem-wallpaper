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
    float4 colorFrom = textureFrom.Sample(samplerState, input.tex);
    float4 colorTo = textureTo.Sample(samplerState, input.tex);
    return lerp(colorFrom, colorTo, progress);
}
