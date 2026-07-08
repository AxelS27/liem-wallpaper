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
    float2 offsetTex = input.tex;
    // Slide right-to-left
    if (offsetTex.x < 1.0 - progress) {
        offsetTex.x += progress;
        return textureFrom.Sample(samplerState, offsetTex);
    } else {
        offsetTex.x -= (1.0 - progress);
        return textureTo.Sample(samplerState, offsetTex);
    }
}
