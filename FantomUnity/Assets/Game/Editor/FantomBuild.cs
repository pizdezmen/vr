using System;
using System.IO;
using System.Reflection;
using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace Fantom.EditorTools
{
    /// <summary>
    /// Настройка проекта и сборка для шлема одной командой.
    /// В Unity: меню Fantom. Из командной строки: -executeMethod Fantom.EditorTools.FantomBuild.BuildApk
    /// </summary>
    public static class FantomBuild
    {
        const string ScenePath = "Assets/Game/Scenes/Fight.unity";
        const string UrpPath = "Assets/Game/Settings/QuestURP.asset";
        const string RendererPath = "Assets/Game/Settings/QuestURP_Renderer.asset";
        const string ApkPath = "Builds/Fantom.apk";

        [MenuItem("Fantom/1. Настроить проект и сцену")]
        public static void Setup()
        {
            SetupPlayer();
            SetupPipeline();
            SetupScene();
            AssetDatabase.SaveAssets();
            Debug.Log("[Fantom] Setup done.");
        }

        [MenuItem("Fantom/2. Собрать игру для шлема (APK)")]
        public static void BuildApkMenu()
        {
            Build();
        }

        /// <summary>Для запуска без окна Unity: настраивает, собирает и выходит.</summary>
        public static void BuildApk()
        {
            int code = 1;
            try
            {
                code = Build() ? 0 : 1;
            }
            catch (Exception e)
            {
                Debug.LogError("[Fantom] Build failed: " + e);
            }
            EditorApplication.Exit(code);
        }

        /// <summary>Для запуска без окна Unity: только настройка и проверка, что код собирается.</summary>
        public static void SetupBatch()
        {
            int code = 1;
            try
            {
                Setup();
                code = 0;
            }
            catch (Exception e)
            {
                Debug.LogError("[Fantom] Setup failed: " + e);
            }
            EditorApplication.Exit(code);
        }

        static bool Build()
        {
            Setup();
            Directory.CreateDirectory("Builds");
            var options = new BuildPlayerOptions
            {
                scenes = new[] { ScenePath },
                locationPathName = ApkPath,
                target = BuildTarget.Android,
                options = BuildOptions.None,
            };
            BuildReport report = BuildPipeline.BuildPlayer(options);
            BuildSummary summary = report.summary;
            Debug.Log("[Fantom] Build result: " + summary.result + ", size " + summary.totalSize / (1024 * 1024)
                + " MB, errors " + summary.totalErrors + ", time " + summary.totalTime);
            return summary.result == BuildResult.Succeeded;
        }

        static void SetupPlayer()
        {
            PlayerSettings.companyName = "Fantom";
            PlayerSettings.productName = "Fantom";
            PlayerSettings.SetApplicationIdentifier(UnityEditor.Build.NamedBuildTarget.Android, "com.fantomvr.game");
            PlayerSettings.SetScriptingBackend(UnityEditor.Build.NamedBuildTarget.Android, ScriptingImplementation.IL2CPP);
            PlayerSettings.Android.targetArchitectures = AndroidArchitecture.ARM64;
            PlayerSettings.colorSpace = ColorSpace.Linear;
        }

        static void SetupPipeline()
        {
            Directory.CreateDirectory("Assets/Game/Settings");
            var asset = AssetDatabase.LoadAssetAtPath<UniversalRenderPipelineAsset>(UrpPath);
            if (asset == null)
            {
                ScriptableRendererData renderer = null;
                try
                {
                    MethodInfo create = typeof(UniversalRenderPipelineAsset).GetMethod("CreateRendererAsset",
                        BindingFlags.Static | BindingFlags.NonPublic | BindingFlags.Public);
                    if (create != null)
                    {
                        renderer = create.Invoke(null,
                            new object[] { RendererPath, RendererType.UniversalRenderer, false, "Renderer" }) as ScriptableRendererData;
                    }
                }
                catch (Exception e)
                {
                    Debug.LogWarning("[Fantom] CreateRendererAsset: " + e.Message);
                }
                if (renderer == null)
                {
                    renderer = ScriptableObject.CreateInstance<UniversalRendererData>();
                    AssetDatabase.CreateAsset(renderer, RendererPath);
                }
                asset = UniversalRenderPipelineAsset.Create(renderer);
                AssetDatabase.CreateAsset(asset, UrpPath);
            }

            // Настройки под шлем: сглаживание 4x, без HDR, без лишних текстур, без теней и постобработки.
            asset.msaaSampleCount = 4;
            asset.supportsHDR = false;
            asset.supportsCameraDepthTexture = false;
            asset.supportsCameraOpaqueTexture = false;
            asset.renderScale = 1f;
            asset.shadowDistance = 0f;
            var so = new SerializedObject(asset);
            SetBool(so, "m_MainLightShadowsSupported", false);
            SetBool(so, "m_AdditionalLightShadowsSupported", false);
            SetBool(so, "m_SupportsLightCookies", false);
            SetInt(so, "m_AdditionalLightsRenderingMode", 0);
            so.ApplyModifiedPropertiesWithoutUndo();
            EditorUtility.SetDirty(asset);

            GraphicsSettings.defaultRenderPipeline = asset;
            int current = QualitySettings.GetQualityLevel();
            for (int i = 0; i < QualitySettings.names.Length; i++)
            {
                QualitySettings.SetQualityLevel(i, false);
                QualitySettings.renderPipeline = asset;
                QualitySettings.antiAliasing = 4;
                QualitySettings.vSyncCount = 0;
            }
            QualitySettings.SetQualityLevel(current, false);
        }

        static void SetBool(SerializedObject so, string name, bool value)
        {
            SerializedProperty p = so.FindProperty(name);
            if (p != null) p.boolValue = value;
        }

        static void SetInt(SerializedObject so, string name, int value)
        {
            SerializedProperty p = so.FindProperty(name);
            if (p != null) p.intValue = value;
        }

        static void SetupScene()
        {
            if (!File.Exists(ScenePath))
            {
                Directory.CreateDirectory(Path.GetDirectoryName(ScenePath));
                var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);

                string[] guids = AssetDatabase.FindAssets("OVRCameraRig t:Prefab",
                    new[] { "Packages/com.meta.xr.sdk.core/Prefabs" });
                GameObject prefab = null;
                foreach (string guid in guids)
                {
                    string path = AssetDatabase.GUIDToAssetPath(guid);
                    if (Path.GetFileNameWithoutExtension(path) == "OVRCameraRig")
                    {
                        prefab = AssetDatabase.LoadAssetAtPath<GameObject>(path);
                    }
                }
                if (prefab == null) throw new Exception("OVRCameraRig prefab not found");
                PrefabUtility.InstantiatePrefab(prefab);

                new GameObject("Game").AddComponent<FightGame>();
                EditorSceneManager.SaveScene(scene, ScenePath);
            }
            EditorBuildSettings.scenes = new[] { new EditorBuildSettingsScene(ScenePath, true) };
        }
    }
}
