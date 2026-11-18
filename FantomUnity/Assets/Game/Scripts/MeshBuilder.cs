using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace Fantom
{
    /// <summary>Собирает сетку из коробок и плоских фигур. Используется один раз при запуске.</summary>
    public sealed class MeshBuilder
    {
        public readonly List<Vector3> Positions = new List<Vector3>();
        public readonly List<Vector3> Normals = new List<Vector3>();
        public readonly List<Color> Colors = new List<Color>();
        public readonly List<int> Indices = new List<int>();

        static readonly Vector3[] FaceNormals =
        {
            Vector3.right, Vector3.left, Vector3.up, Vector3.down, Vector3.forward, Vector3.back,
        };

        /// <summary>Два вектора вдоль грани, подобранные так, чтобы грань смотрела наружу.</summary>
        public static void FaceAxes(int face, out Vector3 n, out Vector3 u, out Vector3 v)
        {
            n = FaceNormals[face];
            u = Mathf.Abs(n.y) > 0.5f ? Vector3.right : Vector3.up;
            v = Vector3.Cross(n, u);
        }

        public void Box(Vector3 center, Vector3 size, Color color)
        {
            Box(Matrix4x4.TRS(center, Quaternion.identity, size), color);
        }

        public void Box(Matrix4x4 m, Color color)
        {
            for (int f = 0; f < 6; f++)
            {
                FaceAxes(f, out Vector3 n, out Vector3 u, out Vector3 v);
                int start = Positions.Count;
                Positions.Add(m.MultiplyPoint3x4((n - u - v) * 0.5f));
                Positions.Add(m.MultiplyPoint3x4((n + u - v) * 0.5f));
                Positions.Add(m.MultiplyPoint3x4((n + u + v) * 0.5f));
                Positions.Add(m.MultiplyPoint3x4((n - u + v) * 0.5f));
                Vector3 wn = m.MultiplyVector(n).normalized;
                for (int i = 0; i < 4; i++)
                {
                    Normals.Add(wn);
                    Colors.Add(color);
                }
                Quad(start);
            }
        }

        /// <summary>Четыре уже добавленные вершины по кругу превращаем в два треугольника.</summary>
        public void Quad(int start)
        {
            // В Unity лицевая сторона обходится по часовой стрелке.
            Indices.Add(start); Indices.Add(start + 2); Indices.Add(start + 1);
            Indices.Add(start); Indices.Add(start + 3); Indices.Add(start + 2);
        }

        public int Vertex(Vector3 p, Vector3 n, Color c)
        {
            Positions.Add(p);
            Normals.Add(n);
            Colors.Add(c);
            return Positions.Count - 1;
        }

        public Mesh ToMesh(string name)
        {
            var mesh = new Mesh { name = name };
            if (Positions.Count > 65000) mesh.indexFormat = IndexFormat.UInt32;
            mesh.SetVertices(Positions);
            mesh.SetNormals(Normals);
            mesh.SetColors(Colors);
            mesh.SetTriangles(Indices, 0, true);
            mesh.UploadMeshData(true);
            return mesh;
        }

        public static Material Material(string shader)
        {
            var s = Resources.Load<Shader>("Shaders/" + shader);
            return new Material(s) { name = shader };
        }

        public static GameObject Object(string name, Transform parent, Mesh mesh, Material material)
        {
            var go = new GameObject(name);
            go.transform.SetParent(parent, false);
            go.AddComponent<MeshFilter>().sharedMesh = mesh;
            var r = go.AddComponent<MeshRenderer>();
            r.sharedMaterial = material;
            r.shadowCastingMode = ShadowCastingMode.Off;
            r.receiveShadows = false;
            r.lightProbeUsage = LightProbeUsage.Off;
            r.reflectionProbeUsage = ReflectionProbeUsage.Off;
            r.motionVectorGenerationMode = MotionVectorGenerationMode.ForceNoMotion;
            r.allowOcclusionWhenDynamic = false;
            return go;
        }
    }
}
