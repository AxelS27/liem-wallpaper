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
    float2 uv = input.tex;
    
    float blockY = floor(uv.y * 30.0);
    float blockNoise = hash(float2(blockY, progress * 10.0));
    
    float xOffset = 0.0;
    if (blockNoise < progress * 0.4) {
        xOffset = (blockNoise - 0.5) * 0.1 * sin(progress * 3.1415);
    }
    
    uv.x += xOffset;
    uv = clamp(uv, 0.0, 1.0);

    float aspect = ddy(input.tex.y) / ddx(input.tex.x);
    float2 uvFrom = get_fill_uv(uv, textureFrom, aspect);
    float2 uvTo = get_fill_uv(uv, textureTo, aspect);

    float4 colFrom = textureFrom.Sample(samplerState, uvFrom);
    float4 colTo = textureTo.Sample(samplerState, uvTo);
    
    if (abs(xOffset) > 0.01) {
        colFrom.r = textureFrom.Sample(samplerState, uvFrom + float2(0.01, 0.0)).r;
        colFrom.b = textureFrom.Sample(samplerState, uvFrom - float2(0.01, 0.0)).b;
        colTo.r = textureTo.Sample(samplerState, uvTo + float2(0.01, 0.0)).r;
        colTo.b = textureTo.Sample(samplerState, uvTo - float2(0.01, 0.0)).b;
    }

    return lerp(colFrom, colTo, progress);
}
