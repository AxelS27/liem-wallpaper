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
    if (input.tex.x <= progress) {
        float2 offsetTex = input.tex;
        offsetTex.x += (1.0 - progress);
        float2 uvTo = get_fill_uv(offsetTex, textureTo, aspect);
        return textureTo.Sample(samplerState, uvTo);
    } else {
        float2 uvFrom = get_fill_uv(input.tex, textureFrom, aspect);
        return textureFrom.Sample(samplerState, uvFrom);
    }
}
