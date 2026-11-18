using UnityEngine;

namespace Fantom
{
    /// <summary>
    /// Тренировочное измерение: тёмная пустота, светящаяся сетка пола,
    /// далёкие столбы света и медленно плывущие пиксели. Всего три отрисовки.
    /// </summary>
    public sealed class Arena
    {
        readonly Transform dust;

        public Arena(Transform parent)
        {
            var root = new GameObject("Arena").transform;
            root.SetParent(parent, false);

            // Пол.
            var floor = new MeshBuilder();
            floor.Box(new Vector3(0f, -0.005f, 0f), new Vector3(120f, 0.01f, 120f), Color.white);
            MeshBuilder.Object("Floor", root, floor.ToMesh("Floor"), MeshBuilder.Material("GridFloor"));

            // Столбы света по кругу.
            var glow = MeshBuilder.Material("Glow");
            var pillars = new MeshBuilder();
            const int pillarCount = 18;
            for (int i = 0; i < pillarCount; i++)
            {
                float angle = i / (float)pillarCount * Mathf.PI * 2f + 0.17f;
                float radius = 16f + (i * 7 % 5) * 3f;
                float height = 6f + (i * 13 % 7) * 3f;
                float width = 0.3f + (i * 5 % 3) * 0.25f;
                float bright = 0.05f + (i * 11 % 5) * 0.02f;
                var color = new Color(0.04f + (i * 3 % 4) * 0.02f, 0.22f, 1f, bright);
                pillars.Box(
                    new Vector3(Mathf.Cos(angle) * radius, height / 2f, Mathf.Sin(angle) * radius),
                    new Vector3(width, height, width), color);
            }
            MeshBuilder.Object("Pillars", root, pillars.ToMesh("Pillars"), glow);

            // Плывущие пиксели.
            var random = new System.Random(11);
            var dustMesh = new MeshBuilder();
            for (int i = 0; i < 350; i++)
            {
                float angle = (float)random.NextDouble() * Mathf.PI * 2f;
                float radius = 2.5f + (float)random.NextDouble() * 14f;
                float size = 0.02f + (float)random.NextDouble() * 0.025f;
                var p = new Vector3(Mathf.Cos(angle) * radius, 0.2f + (float)random.NextDouble() * 7f,
                    Mathf.Sin(angle) * radius);
                dustMesh.Box(p, new Vector3(size, size, size), new Color(0.2f, 0.55f, 1f, 0.35f));
            }
            dust = MeshBuilder.Object("Dust", root, dustMesh.ToMesh("Dust"), glow).transform;
        }

        public void Update(float time)
        {
            dust.localRotation = Quaternion.Euler(0f, time * 0.6f, 0f);
            dust.localPosition = new Vector3(0f, Mathf.Sin(time * 0.2f) * 0.2f, 0f);
        }
    }
}
