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

float hash(float2 p) {
    return frac(sin(dot(p, float2(127.1, 311.7))) * 43758.5453123);
}

float4 main(PS_INPUT input) : SV_Target {
    float2 uv = input.tex;
    
    float blockY = floor(uv.y * 30.0);
    float blockNoise = hash(float2(blockY, progress * 10.0));
    
    float xOffset = 0.0;
    if (blockNoise < progress * 0.4) {
        xOffset = (blockNoise - 0.5) * 0.1 * sin(progress * 3.1415);
    }
    
    uv.x += xOffset;
    uv = clamp(uv, 0.0, 1.0);

    float4 colFrom = textureFrom.Sample(samplerState, uv);
    float4 colTo = textureTo.Sample(samplerState, uv);
    
    if (abs(xOffset) > 0.01) {
        colFrom.r = textureFrom.Sample(samplerState, uv + float2(0.01, 0.0)).r;
        colFrom.b = textureFrom.Sample(samplerState, uv - float2(0.01, 0.0)).b;
        colTo.r = textureTo.Sample(samplerState, uv + float2(0.01, 0.0)).r;
        colTo.b = textureTo.Sample(samplerState, uv - float2(0.01, 0.0)).b;
    }

    return lerp(colFrom, colTo, progress);
}
