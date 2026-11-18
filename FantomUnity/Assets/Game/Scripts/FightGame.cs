using UnityEngine;

namespace Fantom
{
    /// <summary>
    /// Бой с фантомом и забег на очки.
    /// Фантом замахивается (клинок раскаляется) -> игрок подставляет щит или отбивает мечом
    /// -> фантом раскрывается -> игрок бьёт. Любое попадание мечом рассыпает фантома,
    /// но чистый взмах, слабое место, ответный удар и серия без пропусков дают больше очков.
    /// </summary>
    public sealed class FightGame : MonoBehaviour
    {
        enum State { Waiting, Ready, Assemble, Idle, Windup, Swing, Open, Recover, Dissolve, Results }

        const string TiltKey = "bladeTilt";
        static readonly int IntensityId = Shader.PropertyToID("_Intensity");

        Transform head, swordHand, shieldHand;
        Arena arena;
        Phantom phantom;
        Board board;
        SoundBank sound;

        Transform sword, shield;
        Material shieldMaterial, flashMaterial;
        MeshRenderer flashRenderer;
        Transform flash;
        float bladeTilt;

        State state = State.Waiting;
        float stateTime, time, pause, currentWindup;
        Attack attack = Phantom.Attacks[0];
        bool placed;

        // Меч и щит в этом кадре.
        Vector3 headPos, swordBase, swordTip, prevTip, tipVelocity, shieldPos, dangerBase, dangerTip, hitPoint;
        bool hasPrevTip;
        float swingPath;
        float shieldFlash, flashOpacity;
        int guardHand;          // 0 нет, 1 щит, 2 меч
        int openedBy;           // чем был отбит последний удар
        float guardSince = -1f;

        // Забег.
        float runLeft;
        int score, kills, blocks, parries, misses, streak, bestStreak, weakHits, cleanHits;
        int lastReaction = -1, bestReaction = -1;
        string message = "";

        // Кадры в секунду и вибрация.
        float fpsTimer;
        int fpsFrames, fps, shownSecond = -1;
        float hapticLeft, hapticRight;

        void Start()
        {
            var rig = FindAnyObjectByType<OVRCameraRig>();
            if (rig == null)
            {
                Debug.LogError("[Fantom] OVRCameraRig not found in the scene.");
                enabled = false;
                return;
            }
            head = rig.centerEyeAnchor;
            swordHand = rig.rightHandAnchor;
            shieldHand = rig.leftHandAnchor;

            foreach (var cam in rig.GetComponentsInChildren<Camera>())
            {
                cam.clearFlags = CameraClearFlags.SolidColor;
                cam.backgroundColor = new Color(0.001f, 0.003f, 0.012f, 1f);
                cam.farClipPlane = 120f;
                cam.nearClipPlane = 0.05f;
                cam.allowHDR = false;
            }

            SetupHeadset();

            arena = new Arena(transform);
            phantom = new Phantom(transform);
            board = new Board(transform);
            sound = new SoundBank(head);
            bladeTilt = PlayerPrefs.GetFloat(TiltKey, Tuning.BladeTiltDefault);
            BuildSword();
            BuildShield();
            BuildFlash();
        }

        static void SetupHeadset()
        {
            try
            {
                // Высота считается от пола комнаты: пол игры совпадает с настоящим полом.
                if (OVRManager.instance != null)
                {
                    OVRManager.instance.trackingOriginType = OVRManager.TrackingOrigin.FloorLevel;
                }
                OVRManager.foveatedRenderingLevel = OVRManager.FoveatedRenderingLevel.High;
                OVRManager.useDynamicFoveatedRendering = true;
                if (OVRManager.display != null)
                {
                    float[] rates = OVRManager.display.displayFrequenciesAvailable;
                    for (int i = 0; i < rates.Length; i++)
                    {
                        if (Mathf.Abs(rates[i] - Tuning.TargetRefreshRate) < 1f)
                        {
                            OVRManager.display.displayFrequency = rates[i];
                        }
                    }
                }
            }
            catch (System.Exception e)
            {
                Debug.LogWarning("[Fantom] Headset setup: " + e.Message);
            }
        }

        // ---------- Меч, щит, вспышка ----------

        void BuildSword()
        {
            var solid = new MeshBuilder();
            var steel = new Color(0.03f, 0.06f, 0.14f);
            solid.Box(new Vector3(0f, 0f, -0.02f), new Vector3(0.03f, 0.03f, 0.16f), steel);          // рукоять
            solid.Box(new Vector3(0f, 0f, 0.07f), new Vector3(0.16f, 0.024f, 0.024f), steel);        // гарда
            solid.Box(new Vector3(0f, 0f, 0.08f + Tuning.PlayerBlade / 2f),
                new Vector3(0.034f, 0.008f, Tuning.PlayerBlade), new Color(0.55f, 0.85f, 1.6f));       // клинок
            sword = MeshBuilder.Object("Sword", swordHand, solid.ToMesh("Sword"), MeshBuilder.Material("Solid")).transform;

            var glow = new MeshBuilder();
            glow.Box(new Vector3(0f, 0f, 0.08f + Tuning.PlayerBlade / 2f),
                new Vector3(0.07f, 0.024f, Tuning.PlayerBlade + 0.03f), new Color(0.05f, 0.25f, 1f, 0.3f));
            MeshBuilder.Object("SwordGlow", sword, glow.ToMesh("SwordGlow"), MeshBuilder.Material("Glow"));

            ApplyTilt();
            sword.gameObject.SetActive(false);
        }

        void ApplyTilt()
        {
            sword.localRotation = Quaternion.Euler(-bladeTilt, 0f, 0f);
        }

        void BuildShield()
        {
            // Шестиугольный силовой щит: полупрозрачное поле, яркий обод и маленькое кольцо в центре.
            var b = new MeshBuilder();
            var field = new Color(0.1f, 0.4f, 1f, 0.16f);
            var rim = new Color(0.4f, 0.7f, 1f, 0.8f);
            float r = Tuning.ShieldRadius;
            int center = b.Vertex(Vector3.zero, Vector3.forward, field);
            for (int i = 0; i < 6; i++)
            {
                Vector3 p0 = HexPoint(i, 1f), p1 = HexPoint(i + 1, 1f);
                int a = b.Vertex(p0 * r, Vector3.forward, field);
                int c = b.Vertex(p1 * r, Vector3.forward, field);
                b.Indices.Add(center); b.Indices.Add(a); b.Indices.Add(c);
                Ring(b, p0, p1, r - 0.014f, r, rim);
                Ring(b, p0, p1, 0.05f, 0.058f, rim);
            }
            shieldMaterial = MeshBuilder.Material("Glow");
            shield = MeshBuilder.Object("Shield", shieldHand, b.ToMesh("Shield"), shieldMaterial).transform;
            // Щит смотрит туда же, куда указывает контроллер.
            shield.localPosition = new Vector3(0f, 0f, 0.1f);
            shield.gameObject.SetActive(false);
        }

        static Vector3 HexPoint(int i, float radius)
        {
            float angle = i * Mathf.PI / 3f + Mathf.PI / 6f;
            return new Vector3(Mathf.Cos(angle) * radius, Mathf.Sin(angle) * radius, 0f);
        }

        static void Ring(MeshBuilder b, Vector3 p0, Vector3 p1, float inner, float outer, Color color)
        {
            int start = b.Vertex(p0 * inner, Vector3.forward, color);
            b.Vertex(p1 * inner, Vector3.forward, color);
            b.Vertex(p1 * outer, Vector3.forward, color);
            b.Vertex(p0 * outer, Vector3.forward, color);
            b.Quad(start);
        }

        void BuildFlash()
        {
            // Красная вспышка вокруг головы, когда игрок пропустил удар.
            var b = new MeshBuilder();
            b.Box(Vector3.zero, Vector3.one * 0.6f, new Color(1f, 0.05f, 0.08f, 1f));
            flashMaterial = MeshBuilder.Material("Glow");
            flashMaterial.SetFloat("_ZTest", (float)UnityEngine.Rendering.CompareFunction.Always);
            flashMaterial.renderQueue = 4000;
            var go = MeshBuilder.Object("HurtFlash", transform, b.ToMesh("HurtFlash"), flashMaterial);
            flash = go.transform;
            flashRenderer = go.GetComponent<MeshRenderer>();
            flashRenderer.enabled = false;
        }

        // ---------- Вспомогательное ----------

        /// <summary>Расстояние между двумя отрезками (p1-q1 и p2-q2) и ближайшая точка на первом.</summary>
        static float SegmentDistance(Vector3 p1, Vector3 q1, Vector3 p2, Vector3 q2, out Vector3 c1)
        {
            Vector3 d1 = q1 - p1, d2 = q2 - p2, r = p1 - p2;
            float a = Vector3.Dot(d1, d1), e = Vector3.Dot(d2, d2), f = Vector3.Dot(d2, r);
            float s = 0f, t = 0f;
            if (a > 1e-8f && e > 1e-8f)
            {
                float b = Vector3.Dot(d1, d2), c = Vector3.Dot(d1, r);
                float denom = a * e - b * b;
                s = denom > 1e-8f ? Mathf.Clamp01((b * f - c * e) / denom) : 0f;
                t = (b * s + f) / e;
                if (t < 0f) { t = 0f; s = Mathf.Clamp01(-c / a); }
                else if (t > 1f) { t = 1f; s = Mathf.Clamp01((b - c) / a); }
            }
            else if (e > 1e-8f) t = Mathf.Clamp01(f / e);
            else if (a > 1e-8f) s = Mathf.Clamp01(-Vector3.Dot(d1, r) / a);
            c1 = p1 + d1 * s;
            return Vector3.Distance(c1, p2 + d2 * t);
        }

        void Pulse(bool left, float strength, float seconds)
        {
            OVRInput.SetControllerVibration(1f, strength, left ? OVRInput.Controller.LTouch : OVRInput.Controller.RTouch);
            if (left) hapticLeft = seconds; else hapticRight = seconds;
        }

        void UpdateHaptics(float dt)
        {
            if (hapticLeft > 0f && (hapticLeft -= dt) <= 0f)
                OVRInput.SetControllerVibration(0f, 0f, OVRInput.Controller.LTouch);
            if (hapticRight > 0f && (hapticRight -= dt) <= 0f)
                OVRInput.SetControllerVibration(0f, 0f, OVRInput.Controller.RTouch);
        }

        void SetState(State next)
        {
            state = next;
            stateTime = 0f;
        }

        float WindupTime => Mathf.Max(Tuning.WindupMin, Tuning.WindupStart - Tuning.WindupStep * streak);
        float Multiplier => Mathf.Min(Tuning.StreakMax, 1f + Tuning.StreakStep * streak);

        // ---------- Табло ----------

        void Say(string text)
        {
            message = text;
            DrawBoard();
        }

        void DrawBoard()
        {
            string fpsLine = "Кадров в секунду: " + fps + "     Наклон клинка: " + Mathf.RoundToInt(bladeTilt) + "°";
            if (state == State.Ready || state == State.Waiting)
            {
                board.Set("ТРЕНИРОВКА МЕЧНИКА", "Готов?", "Нажми курок, чтобы начать",
                    "Щит в левой руке, меч в правой",
                    "Раскалённый клинок фантома = сейчас ударит",
                    "Правый стик вверх и вниз меняет наклон клинка.   " + fpsLine);
                return;
            }
            if (state == State.Results)
            {
                board.Set("ЗАБЕГ ОКОНЧЕН. ОЦЕНКА: " + Rank(), score.ToString(), "Нажми курок, чтобы сыграть ещё",
                    "Фантомов: " + kills + "     Лучшая серия: " + bestStreak + "     Пропущено: " + misses,
                    "Слабых мест: " + weakHits + "     Чистых взмахов: " + cleanHits
                        + "     Блоков: " + blocks + "     Парирований: " + parries,
                    "Лучшая реакция: " + Ms(bestReaction) + "     " + fpsLine);
                return;
            }
            int seconds = Mathf.CeilToInt(runLeft);
            board.Set("ОЧКИ", score.ToString(), message,
                "Время: " + seconds / 60 + ":" + (seconds % 60).ToString("00")
                    + "     Серия: " + streak + "  (×" + Multiplier.ToString("0.0") + ")",
                "Фантомов: " + kills + "     Блоков: " + (blocks + parries) + "     Пропущено: " + misses,
                "Реакция: " + Ms(lastReaction) + "  (лучшая " + Ms(bestReaction) + ")     " + fpsLine);
        }

        static string Ms(int value) => value < 0 ? "—" : value + " мс";

        string Rank()
        {
            if (score >= Tuning.RankS) return "S";
            if (score >= Tuning.RankA) return "A";
            if (score >= Tuning.RankB) return "B";
            if (score >= Tuning.RankC) return "C";
            return "D";
        }

        // ---------- Ход боя ----------

        /// <summary>Ставим фантома и табло перед игроком.</summary>
        void PlaceArena()
        {
            Vector3 dir = head.forward;
            dir.y = 0f;
            if (dir.sqrMagnitude < 0.0001f) dir = Vector3.forward;
            dir.Normalize();
            float scale = Mathf.Clamp(headPos.y / 1.62f, 0.6f, 1.25f);
            phantom.Root.localScale = Vector3.one * scale;
            Vector3 p = headPos + dir * (Tuning.FightDistance * scale);
            p.y = 0f;
            phantom.Root.position = p;
            FacePlayer(1f);

            Vector3 boardPos = headPos + dir * 4.2f;
            boardPos.y = headPos.y + 1.35f;
            board.Root.position = boardPos;
            board.Root.rotation = Quaternion.LookRotation(boardPos - headPos);
            board.SetVisible(true);
            sword.gameObject.SetActive(true);
            shield.gameObject.SetActive(true);
            placed = true;
        }

        /// <summary>Фантом поворачивается к игроку и держит дистанцию.</summary>
        void FacePlayer(float k)
        {
            Transform root = phantom.Root;
            Vector3 away = root.position - headPos;
            away.y = 0f;
            if (away.sqrMagnitude < 0.01f) return;
            Vector3 goal = headPos + away.normalized * (Tuning.FightDistance * root.localScale.x);
            goal.y = root.position.y;
            root.position = Vector3.Lerp(root.position, goal, k);
            Vector3 look = headPos - root.position;
            look.y = 0f;
            root.rotation = Quaternion.LookRotation(look);
        }

        void StartRun()
        {
            runLeft = Tuning.RunSeconds;
            score = kills = blocks = parries = misses = streak = bestStreak = weakHits = cleanHits = 0;
            lastReaction = bestReaction = -1;
            message = "Фантом идёт!";
            SpawnPhantom();
        }

        void FinishRun()
        {
            sound.StopWindup();
            sound.Finish();
            phantom.Hide();
            SetState(State.Results);
            DrawBoard();
        }

        void SpawnPhantom()
        {
            Vector3 p = phantom.Root.position;
            p.y = 0f;
            phantom.Root.position = p;
            FacePlayer(1f);
            phantom.StartAssemble();
            sound.Spawn();
            SetState(State.Assemble);
            DrawBoard();
        }

        /// <summary>Чем игрок сейчас закрыт от удара: 0 ничем, 1 щитом, 2 мечом.</summary>
        int Guarding()
        {
            Vector3 d = dangerTip - dangerBase;
            float k = Mathf.Clamp01(Vector3.Dot(shieldPos - dangerBase, d) / d.sqrMagnitude);
            if (Vector3.Distance(shieldPos, dangerBase + d * k) < Tuning.BlockRange) return 1;
            return SegmentDistance(swordBase, swordTip, dangerBase, dangerTip, out _) < Tuning.ParryRange ? 2 : 0;
        }

        void StartAttack()
        {
            // Выбираем удар, от которого игрок сейчас НЕ закрыт: стоять со щитом на месте не поможет.
            int start = Random.Range(0, Phantom.Attacks.Length);
            for (int i = 0; i < Phantom.Attacks.Length; i++)
            {
                attack = Phantom.Attacks[(start + i) % Phantom.Attacks.Length];
                phantom.BladeSegment(attack.Hit, out dangerBase, out dangerTip);
                if (Guarding() == 0) break;
            }
            guardSince = -1f;
            currentWindup = WindupTime;
            sound.Windup(currentWindup + Tuning.SwingTime);
            SetState(State.Windup);
        }

        /// <summary>Запоминаем момент, когда игрок встал в защиту (от начала замаха).</summary>
        void TrackGuard(float sinceWindup)
        {
            if (Guarding() != 0)
            {
                if (guardSince < 0f) guardSince = sinceWindup;
            }
            else guardSince = -1f;
        }

        void ResolveImpact()
        {
            guardHand = Guarding();
            if (guardHand != 0)
            {
                int reaction = Mathf.RoundToInt((guardSince >= 0f ? guardSince : stateTime + currentWindup) * 1000f);
                lastReaction = reaction;
                bestReaction = bestReaction < 0 ? reaction : Mathf.Min(bestReaction, reaction);
                openedBy = guardHand;
                if (guardHand == 1)
                {
                    blocks++;
                    score += Tuning.ScoreBlock;
                    sound.Block();
                    shieldFlash = 1f;
                    Pulse(true, 1f, 0.09f);
                    Say("Блок! Бей мечом!   +" + Tuning.ScoreBlock);
                }
                else
                {
                    parries++;
                    score += Tuning.ScoreParry;
                    sound.Parry();
                    Pulse(false, 1f, 0.09f);
                    Say("Парирование! Бей!   +" + Tuning.ScoreParry);
                }
                SetState(State.Open);
            }
            else
            {
                misses++;
                streak = 0;
                sound.Hurt();
                Pulse(true, 1f, 0.2f);
                Pulse(false, 1f, 0.2f);
                flashOpacity = 0.55f;
                flashRenderer.enabled = true;
                Say("Пропустил удар. Серия сброшена");
                SetState(State.Recover);
            }
        }

        /// <summary>Проверяем, задел ли меч игрока тело фантома.</summary>
        bool CheckSwordHit()
        {
            Transform root = phantom.Root;
            Vector3 localBase = root.InverseTransformPoint(swordBase);
            Vector3 localTip = root.InverseTransformPoint(swordTip);
            for (int i = 0; i < Phantom.CapsuleR.Length; i++)
            {
                float d = SegmentDistance(localBase, localTip, Phantom.CapsuleA[i], Phantom.CapsuleB[i], out Vector3 c);
                if (d < Phantom.CapsuleR[i])
                {
                    hitPoint = c;
                    return true;
                }
            }
            return false;
        }

        void KillPhantom(float swordSpeed)
        {
            Transform root = phantom.Root;
            Vector3 push = Vector3.ClampMagnitude(root.InverseTransformVector(tipVelocity), 6f);

            // Пройдёт ли клинок через ядро на груди, если продолжит движение.
            Vector3 localBase = root.InverseTransformPoint(swordBase);
            Vector3 localTip = root.InverseTransformPoint(swordTip);
            float coreDistance = SegmentDistance(localBase, localTip, Phantom.Core, Phantom.Core - push * 0.12f, out _);
            bool weak = coreDistance < Tuning.WeakPointRange;
            bool clean = swordSpeed >= Tuning.CleanSwingSpeed && swingPath >= Tuning.CleanSwingPath;
            bool counter = state == State.Open;

            int points = Tuning.ScoreKill;
            string text = "Фантом рассыпался!";
            if (counter)
            {
                points += openedBy == 2 ? Tuning.ScoreParryCounter : Tuning.ScoreCounter;
                text = "Ответный удар!";
            }
            if (clean)
            {
                points += Tuning.ScoreCleanSwing;
                cleanHits++;
                text = "Чистый взмах!";
            }
            if (weak)
            {
                points += Tuning.ScoreWeakPoint;
                weakHits++;
                text = clean ? "Чистый взмах в слабое место!" : "Слабое место!";
            }
            points = Mathf.RoundToInt(points * Multiplier);
            score += points;
            kills++;
            streak++;
            if (streak > bestStreak) bestStreak = streak;

            sound.StopWindup();
            sound.Shatter();
            Pulse(false, weak || clean ? 1f : 0.7f, weak ? 0.2f : 0.14f);
            phantom.StartDissolve(hitPoint, push);
            SetState(State.Dissolve);
            Say(text + "   +" + points);
        }

        void Update()
        {
            float dt = Mathf.Min(Time.deltaTime, 0.05f);
            time += dt;
            arena.Update(time);
            UpdateHaptics(dt);

            // Кадры в секунду: считаем раз в секунду.
            fpsFrames++;
            fpsTimer += Time.unscaledDeltaTime;
            bool secondPassed = false;
            if (fpsTimer >= 1f)
            {
                fps = Mathf.RoundToInt(fpsFrames / fpsTimer);
                fpsFrames = 0;
                fpsTimer = 0f;
                secondPassed = true;
            }

            headPos = head.position;
            if (headPos.y < 0.3f) return; // шлем ещё не отдаёт позицию головы
            if (!placed)
            {
                PlaceArena();
                SetState(State.Ready);
                DrawBoard();
            }

            // Где сейчас меч и щит.
            swordBase = sword.TransformPoint(0f, 0f, 0.1f);
            swordTip = sword.TransformPoint(0f, 0f, 0.08f + Tuning.PlayerBlade);
            shieldPos = shield.position;
            if (hasPrevTip && dt > 0f)
            {
                tipVelocity = Vector3.Lerp(tipVelocity, (swordTip - prevTip) / dt, 0.6f);
            }
            float swordSpeed = tipVelocity.magnitude;
            // Длина текущего взмаха: копится, пока меч движется быстро.
            if (swordSpeed > 1f && hasPrevTip) swingPath += Vector3.Distance(swordTip, prevTip);
            else swingPath = 0f;
            prevTip = swordTip;
            hasPrevTip = true;

            // Затухание вспышек.
            if (flashRenderer.enabled)
            {
                flash.position = headPos;
                flashOpacity -= dt * 1.4f;
                flashMaterial.SetFloat(IntensityId, Mathf.Max(flashOpacity, 0f));
                if (flashOpacity <= 0f) flashRenderer.enabled = false;
            }
            if (shieldFlash > 0f || secondPassed)
            {
                shieldFlash = Mathf.Max(0f, shieldFlash - dt * 4f);
                shieldMaterial.SetFloat(IntensityId, 1f + shieldFlash * 4f);
                shield.localScale = Vector3.one * (1f + shieldFlash * 0.12f);
            }

            bool trigger = OVRInput.GetDown(OVRInput.RawButton.RIndexTrigger)
                || OVRInput.GetDown(OVRInput.RawButton.LIndexTrigger);

            stateTime += dt;
            float t = stateTime;

            if (state == State.Ready || state == State.Results)
            {
                // Подстройка наклона клинка правым стиком.
                float stick = OVRInput.Get(OVRInput.RawAxis2D.RThumbstick).y;
                if (Mathf.Abs(stick) > 0.5f)
                {
                    bladeTilt = Mathf.Clamp(bladeTilt + stick * 40f * dt, -10f, 80f);
                    ApplyTilt();
                    PlayerPrefs.SetFloat(TiltKey, bladeTilt);
                }
                if (secondPassed) DrawBoard();
                if (trigger && t > 0.5f) StartRun();
                return;
            }

            // Забег идёт.
            runLeft -= dt;
            int second = Mathf.CeilToInt(runLeft);
            if (second != shownSecond)
            {
                shownSecond = second;
                DrawBoard();
            }
            if (runLeft <= 0f)
            {
                FinishRun();
                return;
            }

            float bob = Mathf.Sin(time * 2.2f) * 0.015f;
            bool alive = false;

            switch (state)
            {
                case State.Assemble:
                    if (phantom.UpdateFx(dt))
                    {
                        pause = Random.Range(Tuning.PauseMin, Tuning.PauseMax);
                        SetState(State.Idle);
                    }
                    break;

                case State.Idle:
                {
                    alive = true;
                    FacePlayer(Mathf.Min(dt * 3f, 1f));
                    Vector3 p = phantom.Root.position;
                    p.y = bob;
                    phantom.Root.position = p;
                    phantom.SetPose(Phantom.Idle, Phantom.Idle, 0f);
                    if (t >= pause) StartAttack();
                    break;
                }

                case State.Windup:
                {
                    // Замах: рука уходит назад, клинок раскаляется. Это и есть подсказка игроку.
                    alive = true;
                    float k = Mathf.Min(t / (currentWindup * 0.55f), 1f);
                    phantom.SetPose(Phantom.Idle, attack.Wind, 1f - (1f - k) * (1f - k));
                    phantom.SetHeat(Mathf.Min(t / currentWindup, 1f));
                    TrackGuard(t);
                    if (t >= currentWindup) SetState(State.Swing);
                    break;
                }

                case State.Swing:
                {
                    alive = true;
                    float k = Mathf.Min(t / Tuning.SwingTime, 1f);
                    phantom.SetPose(attack.Wind, attack.Hit, k * k);
                    TrackGuard(currentWindup + t);
                    if (k >= 1f) ResolveImpact();
                    break;
                }

                case State.Open:
                {
                    // Фантом отброшен и открыт: мерцает светлым, удар сейчас даёт больше очков.
                    alive = true;
                    float k = Mathf.Min(t / 0.18f, 1f);
                    phantom.SetPose(attack.Hit, Phantom.Open, 1f - (1f - k) * (1f - k));
                    phantom.SetHeat(Mathf.Max(0f, 1f - t * 4f));
                    phantom.SetOpen(1f, time);
                    if (t >= Tuning.OpenTime)
                    {
                        Say("Не успел ответить");
                        SetState(State.Recover);
                    }
                    break;
                }

                case State.Recover:
                {
                    alive = true;
                    float k = Mathf.Min(t / 0.6f, 1f);
                    phantom.SetPose(phantom.Openness > 0f ? Phantom.Open : attack.Hit, Phantom.Idle, k);
                    phantom.SetHeat(Mathf.Max(0f, phantom.Heat - dt * 3f));
                    if (k >= 1f)
                    {
                        phantom.SetOpen(0f, time);
                        phantom.SetHeat(0f);
                        pause = Random.Range(Tuning.PauseMin, Tuning.PauseMax);
                        SetState(State.Idle);
                    }
                    break;
                }

                case State.Dissolve:
                    if (phantom.UpdateFx(dt) && t > 1.3f) SpawnPhantom();
                    break;
            }

            // Любое попадание мечом по целому фантому рассыпает его.
            if (alive && state != State.Dissolve && swordSpeed > Tuning.MinSwingSpeed && CheckSwordHit())
            {
                KillPhantom(swordSpeed);
            }
        }

        void OnDisable()
        {
            OVRInput.SetControllerVibration(0f, 0f, OVRInput.Controller.LTouch);
            OVRInput.SetControllerVibration(0f, 0f, OVRInput.Controller.RTouch);
        }
    }
}
