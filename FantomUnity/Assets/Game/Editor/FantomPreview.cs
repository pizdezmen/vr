using System;
using System.IO;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

namespace Fantom.EditorTools
{
    /// <summary>Снимки фантома и арены без шлема: чтобы проверить вид после правок.</summary>
    public static class FantomPreview
    {
        [MenuItem("Fantom/3. Сделать снимки для проверки вида")]
        public static void Capture()
        {
            EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
            var root = new GameObject("PreviewRoot").transform;
            new Arena(root);
            var phantom = new Phantom(root);
            phantom.Root.rotation = Quaternion.identity;

            var camGo = new GameObject("PreviewCamera");
            var cam = camGo.AddComponent<Camera>();
            cam.clearFlags = CameraClearFlags.SolidColor;
            cam.backgroundColor = new Color(0.001f, 0.003f, 0.012f, 1f);
            cam.fieldOfView = 60f;
            cam.nearClipPlane = 0.05f;
            cam.farClipPlane = 120f;

            Directory.CreateDirectory("Preview");
            phantom.StartAssemble();
            phantom.UpdateFx(5f);
            Shot(cam, new Vector3(0.25f, 1.45f, 1.7f), new Vector3(0f, 1.05f, 0f), "Preview/phantom_idle.png");
            Shot(cam, new Vector3(1.5f, 1.3f, 1.0f), new Vector3(0f, 1.1f, 0f), "Preview/phantom_side.png");

            phantom.SetPose(Phantom.Idle, Phantom.Attacks[0].Wind, 1f);
            phantom.SetHeat(1f);
            Shot(cam, new Vector3(0.25f, 1.45f, 1.7f), new Vector3(0f, 1.2f, 0f), "Preview/phantom_windup.png");

            phantom.StartDissolve(new Vector3(0.1f, 1.25f, 0.1f), new Vector3(-3f, 0.5f, 0f));
            phantom.UpdateFx(0.3f);
            Shot(cam, new Vector3(0.25f, 1.45f, 1.7f), new Vector3(0f, 1.05f, 0f), "Preview/phantom_dissolve.png");

            phantom.Hide();
            Shot(cam, new Vector3(0f, 1.6f, 0f), new Vector3(0f, 1.8f, -6f), "Preview/arena.png");
            Debug.Log("[Fantom] Preview saved.");
        }

        public static void CaptureBatch()
        {
            int code = 1;
            try
            {
                Capture();
                code = 0;
            }
            catch (Exception e)
            {
                Debug.LogError("[Fantom] Preview failed: " + e);
            }
            EditorApplication.Exit(code);
        }

        static void Shot(Camera cam, Vector3 from, Vector3 lookAt, string path)
        {
            cam.transform.position = from;
            cam.transform.LookAt(lookAt);
            var rt = new RenderTexture(1024, 1024, 24, RenderTextureFormat.ARGB32, RenderTextureReadWrite.sRGB);
            rt.antiAliasing = 4;
            cam.targetTexture = rt;
            cam.Render();
            RenderTexture.active = rt;
            var tex = new Texture2D(1024, 1024, TextureFormat.RGB24, false);
            tex.ReadPixels(new Rect(0, 0, 1024, 1024), 0, 0);
            tex.Apply();
            File.WriteAllBytes(path, tex.EncodeToPNG());
            RenderTexture.active = null;
            cam.targetTexture = null;
            UnityEngine.Object.DestroyImmediate(rt);
            UnityEngine.Object.DestroyImmediate(tex);
        }
    }
}
