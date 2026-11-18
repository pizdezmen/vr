using UnityEngine;

namespace Fantom
{
    /// <summary>Звуки без файлов: всё синтезируется один раз при запуске.</summary>
    public sealed class SoundBank
    {
        const int Rate = 44100;
        const float WindupLength = 1.2f;

        readonly AudioSource[] sources;
        readonly AudioSource windupSource;
        readonly AudioClip spawn, block, shatter, hurt, windup, parry, finish;
        readonly System.Random random = new System.Random(3);
        int next;

        public SoundBank(Transform head)
        {
            sources = new AudioSource[5];
            for (int i = 0; i < sources.Length; i++) sources[i] = MakeSource(head, "Sound" + i);
            windupSource = MakeSource(head, "SoundWindup");

            var b = new float[(int)(Rate * 0.8f)];
            Tone(b, 0, 220, 660, 0.7f, 0.12f, 0f);
            Tone(b, 0, 330, 990, 0.7f, 0.08f, 0.08f);
            spawn = Clip("spawn", b);

            b = new float[(int)(Rate * 0.5f)];
            Burst(b, 2, 2500, 0.12f, 0.35f, 0f);
            Tone(b, 1, 1250, 1180, 0.45f, 0.2f, 0f);
            Tone(b, 1, 1870, 1790, 0.35f, 0.12f, 0f);
            Tone(b, 0, 2960, 2900, 0.25f, 0.08f, 0f);
            block = Clip("block", b);

            b = new float[(int)(Rate * 0.5f)];
            Burst(b, 2, 4000, 0.08f, 0.3f, 0f);
            Tone(b, 1, 2100, 2040, 0.45f, 0.2f, 0f);
            Tone(b, 0, 3150, 3100, 0.4f, 0.14f, 0f);
            Tone(b, 0, 4200, 4150, 0.3f, 0.08f, 0f);
            parry = Clip("parry", b);

            b = new float[(int)(Rate * 1.0f)];
            Tone(b, 0, 190, 45, 0.22f, 0.5f, 0f);
            Burst(b, 1, 1800, 0.18f, 0.4f, 0f);
            float[] notes = { 1568, 2093, 2637, 3136, 2349, 3520, 4186, 2794 };
            for (int i = 0; i < 14; i++)
            {
                float note = notes[random.Next(notes.Length)];
                Tone(b, 0, note, note * 0.985f, 0.2f, 0.07f, 0.03f + i * 0.045f + (float)random.NextDouble() * 0.03f);
            }
            Burst(b, 2, 6000, 0.7f, 0.07f, 0.05f);
            shatter = Clip("shatter", b);

            b = new float[(int)(Rate * 0.45f)];
            Tone(b, 2, 130, 40, 0.4f, 0.3f, 0f);
            Burst(b, 0, 500, 0.3f, 0.5f, 0f);
            hurt = Clip("hurt", b);

            b = new float[(int)(Rate * WindupLength)];
            Tone(b, 2, 160, 520, WindupLength, 0.06f, 0f, false);
            Tone(b, 0, 320, 1040, WindupLength, 0.08f, 0f, false);
            windup = Clip("windup", b);

            b = new float[(int)(Rate * 1.3f)];
            Tone(b, 0, 523, 523, 0.5f, 0.14f, 0f);
            Tone(b, 0, 659, 659, 0.5f, 0.14f, 0.15f);
            Tone(b, 0, 784, 784, 0.5f, 0.14f, 0.3f);
            Tone(b, 0, 1047, 1047, 0.8f, 0.16f, 0.45f);
            finish = Clip("finish", b);
        }

        static AudioSource MakeSource(Transform head, string name)
        {
            var go = new GameObject(name);
            go.transform.SetParent(head, false);
            var s = go.AddComponent<AudioSource>();
            s.playOnAwake = false;
            s.spatialBlend = 0f;
            return s;
        }

        static AudioClip Clip(string name, float[] data)
        {
            for (int i = 0; i < data.Length; i++) data[i] = Mathf.Clamp(data[i], -1f, 1f);
            var clip = AudioClip.Create(name, data.Length, 1, Rate, false);
            clip.SetData(data, 0);
            return clip;
        }

        /// <summary>wave: 0 синус, 1 треугольник, 2 пила. Частота плавно идёт от from к to.</summary>
        static void Tone(float[] buffer, int wave, float from, float to, float duration, float volume, float delay,
            bool fade = true)
        {
            int start = (int)(delay * Rate);
            int count = Mathf.Min((int)(duration * Rate), buffer.Length - start);
            double phase = 0;
            float ratio = Mathf.Max(to, 1f) / from;
            for (int i = 0; i < count; i++)
            {
                float t = i / (float)(duration * Rate);
                float freq = from * Mathf.Pow(ratio, t);
                phase += freq / Rate;
                float p = (float)(phase - System.Math.Floor(phase));
                float s = wave == 0 ? Mathf.Sin(p * Mathf.PI * 2f)
                    : wave == 1 ? 4f * Mathf.Abs(p - 0.5f) - 1f
                    : (2f * p - 1f) * 0.6f;
                float gain = fade ? volume * Mathf.Pow(0.005f, t) : volume * Mathf.Min(1f, (1f - t) * 30f);
                gain *= Mathf.Min(1f, i / 80f); // без щелчка в начале
                buffer[start + i] += s * gain;
            }
        }

        /// <summary>Шум через простой фильтр. filter: 0 низкие, 1 середина, 2 высокие частоты.</summary>
        void Burst(float[] buffer, int filter, float frequency, float duration, float volume, float delay)
        {
            int start = (int)(delay * Rate);
            int count = Mathf.Min((int)(duration * Rate), buffer.Length - start);
            float a = 1f - Mathf.Exp(-2f * Mathf.PI * frequency / Rate);
            float low = 0f, low2 = 0f;
            for (int i = 0; i < count; i++)
            {
                float t = i / (float)(duration * Rate);
                float white = (float)random.NextDouble() * 2f - 1f;
                low += a * (white - low);
                low2 += a * 0.3f * (white - low2);
                float s = filter == 0 ? low : filter == 1 ? (low - low2) * 1.5f : white - low;
                buffer[start + i] += s * volume * Mathf.Pow(0.005f, t);
            }
        }

        void Play(AudioClip clip, float volume = 1f)
        {
            var s = sources[next];
            next = (next + 1) % sources.Length;
            s.pitch = 1f;
            s.clip = clip;
            s.volume = volume;
            s.Play();
        }

        public void Spawn() => Play(spawn);
        public void Block() => Play(block);
        public void Parry() => Play(parry);
        public void Shatter() => Play(shatter);
        public void Hurt() => Play(hurt);
        public void Finish() => Play(finish);

        /// <summary>Фантом замахивается: нарастающий тон, который кончается ровно в момент удара.</summary>
        public void Windup(float duration)
        {
            windupSource.clip = windup;
            windupSource.pitch = WindupLength / duration;
            windupSource.volume = 1f;
            windupSource.Play();
        }

        public void StopWindup() => windupSource.Stop();
    }
}
