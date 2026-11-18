// Пол тренировочного измерения: светящаяся сетка, гаснущая вдали, с бегущей волной.
Shader "Fantom/GridFloor"
{
    Properties
    {
        _GridColor ("Grid", Color) = (0.02, 0.17, 1, 1)
        _BaseColor ("Base", Color) = (0.001, 0.003, 0.012, 1)
    }
    SubShader
    {
        Tags { "RenderType" = "Opaque" "Queue" = "Geometry+10" "RenderPipeline" = "UniversalPipeline" }
        Pass
        {
            Name "Forward"
            Cull Back
            ZWrite On

            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma multi_compile_instancing
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            struct Attributes
            {
                float4 positionOS : POSITION;
                UNITY_VERTEX_INPUT_INSTANCE_ID
            };

            struct Varyings
            {
                float4 positionCS : SV_POSITION;
                float2 xz : TEXCOORD0;
                UNITY_VERTEX_OUTPUT_STEREO
            };

            CBUFFER_START(UnityPerMaterial)
                half4 _GridColor;
                half4 _BaseColor;
            CBUFFER_END

            Varyings vert(Attributes v)
            {
                Varyings o;
                UNITY_SETUP_INSTANCE_ID(v);
                UNITY_INITIALIZE_VERTEX_OUTPUT_STEREO(o);
                float3 world = TransformObjectToWorld(v.positionOS.xyz);
                o.xz = world.xz;
                o.positionCS = TransformWorldToHClip(world);
                return o;
            }

            float gridLine(float2 p, float width)
            {
                float2 g = abs(frac(p - 0.5) - 0.5) / max(fwidth(p), 1e-5);
                return 1.0 - min(min(g.x, g.y) / width, 1.0);
            }

            half4 frag(Varyings i) : SV_Target
            {
                float dist = length(i.xz);
                float fine = gridLine(i.xz * 2.0, 1.2) * smoothstep(9.0, 2.0, dist);
                float coarse = gridLine(i.xz * 0.5, 1.6);
                float fade = smoothstep(45.0, 3.0, dist);
                float pulse = 0.5 + 0.5 * sin(dist * 1.2 - _Time.y * 1.5);
                half3 color = _GridColor.rgb * (fine * 0.45 + coarse * (0.6 + 0.4 * pulse));
                return half4(_BaseColor.rgb + color * fade, 1.0);
            }
            ENDHLSL
        }
    }
}
