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

float4 main(PS_INPUT input) : SV_Target {
    float aspect = ddy(input.tex.y) / ddx(input.tex.x);
    if (progress <= 0.0) {
        float2 uvFrom = get_fill_uv(input.tex, textureFrom, aspect);
        return textureFrom.Sample(samplerState, uvFrom);
    }
    if (progress >= 1.0) {
        float2 uvTo = get_fill_uv(input.tex, textureTo, aspect);
        return textureTo.Sample(samplerState, uvTo);
    }

    float2 center = float2(0.5, 0.5);
    
    // Correct for aspect ratio using ddx/ddy to make it a perfect circle
    float2 distUV = input.tex;
    distUV.x = (distUV.x - 0.5) * aspect + 0.5;

    float dist = distance(distUV, center);
    float maxDist = sqrt(0.25 * aspect * aspect + 0.25);
    
    float feather = 0.02;
    float radius = progress * (maxDist + feather);
    float mask = smoothstep(radius, radius - feather, dist);

    float2 uvFrom = get_fill_uv(input.tex, textureFrom, aspect);
    float2 uvTo = get_fill_uv(input.tex, textureTo, aspect);

    float4 colorFrom = textureFrom.Sample(samplerState, uvFrom);
    float4 colorTo = textureTo.Sample(samplerState, uvTo);
    return lerp(colorFrom, colorTo, mask);
}
