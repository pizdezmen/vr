using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace Fantom
{
    /// <summary>Поза руки с мечом: направление руки от плеча и направление клинка.</summary>
    public sealed class Pose
    {
        public readonly Vector3 Arm;
        public readonly Vector3 Blade;

        public Pose(float ax, float ay, float az, float bx, float by, float bz)
        {
            Arm = new Vector3(ax, ay, az).normalized;
            Blade = new Vector3(bx, by, bz).normalized;
        }
    }

    public sealed class Attack
    {
        public readonly string Name;
        public readonly Pose Wind;
        public readonly Pose Hit;

        public Attack(string name, Pose wind, Pose hit)
        {
            Name = name;
            Wind = wind;
            Hit = hit;
        }
    }

    /// <summary>
    /// Фантом: безликий мечник в капюшоне из светящихся синих кубиков, без ног, парит над полом.
    /// Вся фигура это одна сетка и одна отрисовка; сборку и распад считает шейдер.
    /// Локальные оси: +Z смотрит на игрока, +X это лево игрока (правая рука фантома), Y вверх.
    /// </summary>
    public sealed class Phantom
    {
        const float Cell = 0.036f;          // шаг сетки кубиков, м
        const float Cube = Cell * 0.86f;    // размер кубика
        public const float ArmLength = 0.55f;
        public const float BladeLength = 0.9f;
        const float DissolveLife = 0.75f;
        const float WaveSpeed = 2.6f;

        public static readonly Vector3 Shoulder = new Vector3(0.26f, 1.42f, 0f);
        /// <summary>Слабое место: светящееся ядро на груди.</summary>
        public static readonly Vector3 Core = new Vector3(0f, 1.25f, 0.1f);

        // Позы перенесены из браузерного демо (ось X отражена: в Unity она смотрит в другую сторону).
        public static readonly Pose Idle = new Pose(0.25f, -0.75f, 0.6f, -0.25f, 0.9f, 0.35f);
        public static readonly Pose Open = new Pose(0.95f, 0.1f, -0.3f, 0.7f, -0.5f, -0.5f);
        static readonly Pose TopWind = new Pose(0.1f, 0.9f, -0.3f, 0f, 0.6f, -0.8f);
        static readonly Pose TopHit = new Pose(-0.3f, 0.5f, 0.8f, -0.1f, 0.03f, 1f);
        static readonly Pose LeftWind = new Pose(0.9f, 0.25f, -0.2f, 0.6f, 0.5f, -0.6f);
        static readonly Pose LeftHit = new Pose(0.35f, -0.2f, 0.9f, 0.055f, -0.23f, 0.97f);
        static readonly Pose RightWind = new Pose(-0.75f, 0.45f, 0.35f, -0.7f, 0.6f, -0.4f);
        static readonly Pose RightHit = new Pose(-0.55f, -0.2f, 0.85f, -0.467f, -0.193f, 0.847f);

        public static readonly Attack[] Attacks =
        {
            new Attack("top", TopWind, TopHit),
            new Attack("left", LeftWind, LeftHit),
            new Attack("right", RightWind, RightHit),
        };

        // Упрощённое тело для проверки попадания мечом: отрезки с радиусом.
        public static readonly Vector3[] CapsuleA =
        {
            new Vector3(0f, 1.58f, 0f), new Vector3(0f, 0.95f, 0f), new Vector3(0f, 0.42f, 0f),
        };
        public static readonly Vector3[] CapsuleB =
        {
            new Vector3(0f, 1.68f, 0f), new Vector3(0f, 1.4f, 0f), new Vector3(0f, 0.85f, 0f),
        };
        public static readonly float[] CapsuleR = { 0.19f, 0.28f, 0.26f };

        static readonly int PartId = Shader.PropertyToID("_Part");
        static readonly int HitId = Shader.PropertyToID("_Hit");
        static readonly int PushId = Shader.PropertyToID("_Push");
        static readonly int FxModeId = Shader.PropertyToID("_FxMode");
        static readonly int FxTimeId = Shader.PropertyToID("_FxTime");
        static readonly int HeatId = Shader.PropertyToID("_Heat");
        static readonly int OpenId = Shader.PropertyToID("_Open");

        public readonly Transform Root;
        public float Heat { get; private set; }
        public float Openness { get; private set; }

        readonly MeshRenderer renderer;
        readonly Material material;
        readonly Matrix4x4[] parts = { Matrix4x4.identity, Matrix4x4.identity, Matrix4x4.identity };
        Vector3 hand;
        int fx; // 0 нет, 1 сборка, 2 распад
        float fxTime;
        float fxDuration;

        public Phantom(Transform parent)
        {
            material = MeshBuilder.Material("PhantomVoxel");
            var go = MeshBuilder.Object("Phantom", parent, BuildMesh(), material);
            Root = go.transform;
            renderer = go.GetComponent<MeshRenderer>();
            SetPose(Idle, Idle, 0f);
            SetHeat(0f);
            SetOpen(0f, 0f);
            Hide();
        }

        // ---------- Форма ----------

        static bool Ell(float x, float y, float z, float cx, float cy, float cz, float rx, float ry, float rz)
        {
            float dx = (x - cx) / rx, dy = (y - cy) / ry, dz = (z - cz) / rz;
            return dx * dx + dy * dy + dz * dz <= 1f;
        }

        static float Hash(int a, int b)
        {
            unchecked
            {
                uint h = (uint)(a * 374761393 + b * 668265263);
                h = (h ^ (h >> 13)) * 1274126177u;
                return ((h ^ (h >> 16)) & 0xffff) / 65535f;
            }
        }

        static bool BodyInside(int ix, int iy, int iz)
        {
            float x = ix * Cell, y = iy * Cell, z = iz * Cell;
            float ax = Mathf.Abs(x);

            // Тёмный провал на месте лица.
            if (Ell(x, y, z, 0f, 1.585f, 0.11f, 0.085f, 0.105f, 0.115f)) return false;
            // Капюшон с острой макушкой, загнутой назад.
            if (Ell(x, y, z, 0f, 1.6f, -0.01f, 0.14f, 0.17f, 0.15f)) return true;
            if (ax / 0.07f + Mathf.Abs(y - 1.77f) / 0.11f + Mathf.Abs(z + 0.07f) / 0.1f <= 1f) return true;
            // Острые плечи: гранёная «перекладина» и наплечники, задранные наружу и вверх.
            if (ax / 0.36f + Mathf.Abs(y - 1.4f) / 0.13f + Mathf.Abs(z) / 0.17f <= 1f) return true;
            if (ax >= 0.14f && ax <= 0.47f)
            {
                float along = (ax - 0.14f) / 0.33f;            // 0 у шеи, 1 на острие
                float ridge = 1.41f + 0.2f * along;
                if (Mathf.Abs(y - ridge) <= 0.085f * (1f - along) + 0.02f
                    && Mathf.Abs(z) <= 0.14f * (1f - along) + 0.02f) return true;
            }
            // Торс.
            if (Ell(x, y, z, 0f, 1.2f, 0f, 0.2f, 0.28f, 0.125f)) return true;
            // Свободный рукав (левая рука фантома).
            if (Ell(x, y, z, -0.29f, 1.12f, 0.02f, 0.065f, 0.3f, 0.07f)) return true;
            // Мантия: расширяется книзу, подол рваный, ног нет.
            if (y <= 1.05f && y >= 0.36f + Hash(ix, iz) * 0.3f)
            {
                float t = (1.05f - y) / 0.7f;
                float rx = Mathf.Lerp(0.17f, 0.27f, t);
                float rz = Mathf.Lerp(0.115f, 0.19f, t);
                if (x * x / (rx * rx) + z * z / (rz * rz) <= 1f) return true;
            }
            return false;
        }

        static bool ArmInside(int ix, int iy, int iz)
        {
            float x = ix * Cell, y = iy * Cell, z = iz * Cell;
            return Ell(x, y, z, 0f, 0f, ArmLength / 2f, 0.062f, 0.062f, ArmLength / 2f + 0.03f);
        }

        sealed class VoxelData
        {
            public readonly List<Vector3> Centers = new List<Vector3>();
            public readonly List<Vector3> Normals = new List<Vector3>();
            public readonly List<Vector4> Uv0 = new List<Vector4>();
            public readonly List<Vector4> Uv1 = new List<Vector4>();
            public readonly List<Vector4> Uv2 = new List<Vector4>();
            public readonly List<int> Indices = new List<int>();
            readonly System.Random random = new System.Random(7);

            float Rand() => (float)random.NextDouble();

            public void Cube(Vector3 center, int part, int kind, float size)
            {
                var rnd = new Vector4(Rand() * 2f - 1f, Rand() * 2f - 1f, Rand() * 2f - 1f, 0.55f + Rand() * 0.45f);
                var info = new Vector4(kind, Rand(), 0f, 0f);
                for (int f = 0; f < 6; f++)
                {
                    MeshBuilder.FaceAxes(f, out Vector3 n, out Vector3 u, out Vector3 v);
                    int start = Centers.Count;
                    for (int i = 0; i < 4; i++)
                    {
                        float su = (i == 1 || i == 2) ? 1f : -1f;
                        float sv = (i >= 2) ? 1f : -1f;
                        Vector3 corner = (n + u * su + v * sv) * (size * 0.5f);
                        Centers.Add(center);
                        Normals.Add(n);
                        Uv0.Add(new Vector4(corner.x, corner.y, corner.z, part));
                        Uv1.Add(rnd);
                        Uv2.Add(info);
                    }
                    Indices.Add(start); Indices.Add(start + 2); Indices.Add(start + 1);
                    Indices.Add(start); Indices.Add(start + 3); Indices.Add(start + 2);
                }
            }

            public float Next() => Rand();
        }

        delegate bool Inside(int ix, int iy, int iz);

        /// <summary>Оставляем только «кожу» фигуры: кубики на поверхности.</summary>
        static void Voxelize(VoxelData data, Inside inside, int part, Vector3Int min, Vector3Int max)
        {
            for (int ix = min.x; ix <= max.x; ix++)
            for (int iy = min.y; iy <= max.y; iy++)
            for (int iz = min.z; iz <= max.z; iz++)
            {
                if (!inside(ix, iy, iz)) continue;
                bool surface =
                    !inside(ix + 1, iy, iz) || !inside(ix - 1, iy, iz) ||
                    !inside(ix, iy + 1, iz) || !inside(ix, iy - 1, iz) ||
                    !inside(ix, iy, iz + 1) || !inside(ix, iy, iz - 1);
                if (!surface) continue;

                float x = ix * Cell, y = iy * Cell, z = iz * Cell;
                int kind = 0;
                if (part == 0)
                {
                    // Стенки провала под капюшоном: почти чёрные.
                    bool nearCavity = Ell(x, y, z, 0f, 1.585f, 0.11f, 0.13f, 0.15f, 0.16f);
                    bool innerHood = Ell(x, y, z, 0f, 1.6f, -0.01f, 0.1f, 0.13f, 0.11f);
                    if (nearCavity && innerHood) kind = 1;
                    // Слабое место: ромб на груди.
                    else if (z > 0.06f && Mathf.Abs(x) + Mathf.Abs(y - Core.y) <= 0.075f) kind = 2;
                }
                data.Cube(new Vector3(x, y, z), part, kind, Cube);
            }
        }

        static Mesh BuildMesh()
        {
            var data = new VoxelData();
            Voxelize(data, BodyInside, 0, new Vector3Int(-14, 8, -7), new Vector3Int(14, 54, 7));

            // Пиксели, осыпающиеся с подола.
            for (int i = 0; i < 90; i++)
            {
                float angle = data.Next() * Mathf.PI * 2f;
                float radius = Mathf.Sqrt(data.Next());
                var p = new Vector3(Mathf.Cos(angle) * 0.25f * radius, 0.36f + data.Next() * 0.2f,
                    Mathf.Sin(angle) * 0.17f * radius);
                data.Cube(p, 0, 3, Cube);
            }

            Voxelize(data, ArmInside, 1, new Vector3Int(-3, -3, -1), new Vector3Int(3, 3, 17));

            // Клинок и гарда.
            for (float z = -0.06f; z <= BladeLength; z += Cell * 0.75f)
            {
                data.Cube(new Vector3(0f, 0f, z), 2, 4, Cube);
                if (z > 0.06f && z < BladeLength - 0.05f) data.Cube(new Vector3(0f, Cell * 0.7f, z), 2, 4, Cube);
            }
            for (float x = -0.1f; x <= 0.1f; x += Cell) data.Cube(new Vector3(x, 0f, 0.06f), 2, 4, Cube);

            var mesh = new Mesh { name = "PhantomVoxels", indexFormat = IndexFormat.UInt32 };
            mesh.SetVertices(data.Centers);
            mesh.SetNormals(data.Normals);
            mesh.SetUVs(0, data.Uv0);
            mesh.SetUVs(1, data.Uv1);
            mesh.SetUVs(2, data.Uv2);
            mesh.SetTriangles(data.Indices, 0, false);
            // Кубики двигает шейдер, поэтому границы задаём с запасом, чтобы фигуру не отсекало.
            mesh.bounds = new Bounds(new Vector3(0f, 1f, 0f), new Vector3(6f, 6f, 6f));
            mesh.UploadMeshData(true);
            Debug.Log("[Fantom] Phantom cubes: " + data.Centers.Count / 24);
            return mesh;
        }

        // ---------- Поведение ----------

        /// <summary>Поза руки: смесь двух поз, k от 0 до 1.</summary>
        public void SetPose(Pose from, Pose to, float k)
        {
            Vector3 arm = Vector3.Lerp(from.Arm, to.Arm, k);
            if (arm.sqrMagnitude < 0.01f) arm = Vector3.up;
            arm.Normalize();
            Vector3 blade = Vector3.Lerp(from.Blade, to.Blade, k);
            if (blade.sqrMagnitude < 0.01f) blade = Vector3.up;
            blade.Normalize();
            hand = Shoulder + arm * ArmLength;
            parts[1] = Matrix4x4.TRS(Shoulder, Quaternion.FromToRotation(Vector3.forward, arm), Vector3.one);
            parts[2] = Matrix4x4.TRS(hand, Quaternion.FromToRotation(Vector3.forward, blade), Vector3.one);
            material.SetMatrixArray(PartId, parts);
        }

        /// <summary>Опасная (дальняя) часть клинка для заданной позы, в мировых координатах.</summary>
        public void BladeSegment(Pose pose, out Vector3 worldBase, out Vector3 worldTip)
        {
            Vector3 b = Shoulder + pose.Arm * ArmLength;
            Vector3 tip = b + pose.Blade * BladeLength;
            b += pose.Blade * (BladeLength * 0.45f);
            worldBase = Root.TransformPoint(b);
            worldTip = Root.TransformPoint(tip);
        }

        /// <summary>0 холодный клинок, 1 раскалённый (сейчас ударит).</summary>
        public void SetHeat(float heat)
        {
            Heat = heat;
            material.SetFloat(HeatId, heat);
        }

        /// <summary>0 обычный, 1 раскрыт для ответного удара (мерцает светлым).</summary>
        public void SetOpen(float openness, float time)
        {
            Openness = openness;
            float flicker = openness > 0f ? 0.6f + 0.4f * Mathf.Sin(time * 28f) : 0f;
            material.SetFloat(OpenId, openness * flicker * 0.7f);
        }

        /// <summary>Фантом собирается из пикселей снизу вверх.</summary>
        public void StartAssemble()
        {
            SetHeat(0f);
            SetOpen(0f, 0f);
            SetPose(Idle, Idle, 0f);
            StartFx(1, 1.05f);
        }

        /// <summary>Фантом рассыпается. hit: точка попадания, push: скорость меча (в координатах фантома).</summary>
        public void StartDissolve(Vector3 hit, Vector3 push)
        {
            material.SetVector(HitId, hit);
            material.SetVector(PushId, push);
            // Самые дальние от удара кубики: низ подола, макушка и кончик клинка.
            float far = Mathf.Max(
                Vector3.Distance(hit, new Vector3(0f, 0f, 0f)),
                Vector3.Distance(hit, new Vector3(0f, 1.9f, 0f)),
                Vector3.Distance(hit, hand) + BladeLength);
            StartFx(2, far / WaveSpeed + 0.04f + DissolveLife);
        }

        void StartFx(int mode, float duration)
        {
            fx = mode;
            fxTime = 0f;
            fxDuration = duration;
            material.SetFloat(FxModeId, mode);
            material.SetFloat(FxTimeId, 0f);
            renderer.enabled = true;
        }

        /// <summary>Возвращает true, когда эффект закончился.</summary>
        public bool UpdateFx(float dt)
        {
            if (fx == 0) return true;
            fxTime += dt;
            material.SetFloat(FxTimeId, fxTime);
            if (fxTime < fxDuration) return false;
            if (fx == 2) renderer.enabled = false;
            fx = 0;
            material.SetFloat(FxModeId, 0f);
            return true;
        }

        public void Hide()
        {
            fx = 0;
            material.SetFloat(FxModeId, 0f);
            renderer.enabled = false;
        }
    }
}
