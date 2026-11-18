// Фантом целиком: все кубики в одной сетке, одна отрисовка.
// Сборка, распад и падающие пиксели считаются на видеокарте, процессор в кадре не занят.
Shader "Fantom/PhantomVoxel"
{
    Properties
    {
        _BodyColor ("Body", Color) = (0.04, 0.3, 1, 1)
        _OpenColor ("Open", Color) = (0.75, 0.9, 1, 1)
        _BladeCold ("Blade cold", Color) = (0.2, 0.55, 1, 1)
        _BladeHot ("Blade hot", Color) = (1, 0.22, 0.02, 1)
        _CoreColor ("Core", Color) = (0.6, 1, 1, 1)
    }
    SubShader
    {
        Tags { "RenderType" = "Opaque" "Queue" = "Geometry" "RenderPipeline" = "UniversalPipeline" }
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
                float4 positionOS : POSITION;   // центр кубика в координатах своей части
                float3 normalOS : NORMAL;
                float4 uv0 : TEXCOORD0;         // xyz: смещение угла, w: номер части
                float4 uv1 : TEXCOORD1;         // xyz: случайный вектор, w: яркость
                float4 uv2 : TEXCOORD2;         // x: вид кубика, y: случайное 0..1
                UNITY_VERTEX_INPUT_INSTANCE_ID
            };

            struct Varyings
            {
                float4 positionCS : SV_POSITION;
                half3 color : COLOR0;
                UNITY_VERTEX_OUTPUT_STEREO
            };

            CBUFFER_START(UnityPerMaterial)
                half4 _BodyColor;
                half4 _OpenColor;
                half4 _BladeCold;
                half4 _BladeHot;
                half4 _CoreColor;
            CBUFFER_END

            float4x4 _Part[3];   // 0 тело, 1 рука, 2 клинок
            float4 _Hit;         // точка попадания (координаты фантома)
            float4 _Push;        // скорость меча (координаты фантома)
            float _FxMode;       // 0 обычный, 1 сборка, 2 распад
            float _FxTime;
            float _Heat;
            float _Open;

            Varyings vert(Attributes v)
            {
                Varyings o;
                UNITY_SETUP_INSTANCE_ID(v);
                UNITY_INITIALIZE_VERTEX_OUTPUT_STEREO(o);

                int part = (int)(v.uv0.w + 0.5);
                float4x4 pm = _Part[part];
                float3 pos = mul(pm, float4(v.positionOS.xyz, 1.0)).xyz;
                float3 corner = mul((float3x3)pm, v.uv0.xyz);
                float3 nrm = mul((float3x3)pm, v.normalOS);
                float3 rnd = v.uv1.xyz;
                float shade = v.uv1.w;
                float kind = v.uv2.x;
                float r01 = v.uv2.y;

                float scale = 1.0;
                float bright = shade;
                float flash = 0.0;

                // Пиксели, падающие с подола мантии.
                if (kind > 2.5 && kind < 3.5)
                {
                    float f = frac(_Time.y * 0.3 * (0.6 + r01) + r01 * 7.0);
                    pos.y -= f * 0.4;
                    pos.xz += rnd.xz * f * 0.05;
                    scale = (1.0 - f) * 0.85;
                }

                if (_FxMode > 0.5 && _FxMode < 1.5)
                {
                    // Сборка снизу вверх.
                    float t = _FxTime - (pos.y / 1.8 * 0.55 + r01 * 0.1);
                    float k = saturate(t / 0.45);
                    float rest = (1.0 - k) * (1.0 - k);
                    pos += float3(rnd.x, -0.2 - abs(rnd.y) * 0.5, rnd.z) * rest;
                    scale *= k;
                    bright *= 0.6 + (1.0 - k) * 1.2;
                }
                else if (_FxMode > 1.5)
                {
                    // Распад волной от точки попадания.
                    float3 d = pos - _Hit.xyz;
                    float dist = length(d) + 0.001;
                    float t = _FxTime - (dist / 2.6 + r01 * 0.04);
                    if (t > 0.0)
                    {
                        float k = saturate(t / 0.75);
                        float near = saturate(1.0 - dist / 0.5);
                        float outSpeed = 0.35 + shade * 0.6 + near * 1.6;
                        float carry = (0.12 + near * 0.35) * r01;
                        float3 vel = d / dist * outSpeed + _Push.xyz * carry
                            + float3(rnd.x * 0.15, 0.25 + abs(rnd.y) * 0.5, rnd.z * 0.15);
                        float drag = 1.0 - exp(-3.0 * t);
                        pos += vel * drag / 3.0;
                        pos.y += 0.12 * t * t;
                        scale *= (1.0 - k) * (1.0 + 0.6 * saturate(1.0 - t / 0.1));
                        flash = saturate(1.0 - t / 0.12);
                        bright = (bright + flash * 1.2) * (1.0 - k * k);
                    }
                }

                // Цвет.
                half3 base = lerp(_BodyColor.rgb, _OpenColor.rgb, _Open);
                base *= 0.75 + 0.45 * saturate((pos.y - 0.3) / 1.5);          // к подолу темнее
                if (r01 > 0.94) base = half3(0.25, 0.85, 1.0);                // редкие яркие искры
                if (kind > 0.5 && kind < 1.5) base *= 0.07;                   // провал под капюшоном
                if (kind > 1.5 && kind < 2.5)                                 // слабое место
                {
                    base = _CoreColor.rgb * (1.4 + 0.4 * sin(_Time.y * 6.0));
                    bright = max(bright, 1.0) * (_FxMode > 1.5 ? bright : 1.0);
                }
                if (kind > 3.5) base = lerp(_BladeCold.rgb, _BladeHot.rgb, _Heat) * 1.3; // клинок

                float light = 0.6 + 0.6 * saturate(dot(nrm, normalize(float3(0.25, 0.7, 0.65))));
                float shimmer = 0.9 + 0.1 * sin(_Time.y * 3.0 + r01 * 40.0);
                o.color = base * (bright * light * shimmer) + flash;

                o.positionCS = TransformObjectToHClip(pos + corner * scale);
                return o;
            }

            half4 frag(Varyings i) : SV_Target
            {
                return half4(i.color, 1.0);
            }
            ENDHLSL
        }
    }
}
