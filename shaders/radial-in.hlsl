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

    float2 center = float2(0.5, 0.5);
    
    // Correct for aspect ratio using ddx/ddy to make it a perfect circle
    float aspect = ddy(input.tex.y) / ddx(input.tex.x);
    float2 distUV = input.tex;
    distUV.x = (distUV.x - 0.5) * aspect + 0.5;

    float dist = distance(distUV, center);
    float maxDist = sqrt(0.25 * aspect * aspect + 0.25);
    
    float feather = 0.02;
    float radius = progress * (maxDist + feather);
    float mask = smoothstep(radius, radius - feather, dist);

    float4 colorFrom = textureFrom.Sample(samplerState, input.tex);
    float4 colorTo = textureTo.Sample(samplerState, input.tex);
    return lerp(colorFrom, colorTo, mask);
}
