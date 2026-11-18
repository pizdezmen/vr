namespace Fantom
{
    /// <summary>Настройки ощущений. Их удобно крутить после пробы в шлеме.</summary>
    public static class Tuning
    {
        // Меч и щит игрока.
        public const float PlayerBlade = 0.85f;       // длина клинка, м
        public const float BladeTiltDefault = 35f;    // наклон клинка вверх от направления контроллера, градусы
        public const float ShieldRadius = 0.27f;
        public const float BlockRange = 0.25f;        // щит ближе этого к клинку фантома = блок
        public const float ParryRange = 0.09f;        // меч ближе этого к клинку фантома = парирование
        public const float MinSwingSpeed = 1.3f;      // м/с: медленное касание не считается ударом

        // Фантом.
        public const float FightDistance = 1.25f;     // на каком расстоянии он держится
        public const float WindupStart = 0.95f;       // секунд на замах в начале
        public const float WindupMin = 0.38f;         // быстрее этого замах не станет
        public const float WindupStep = 0.05f;        // ускорение замаха за каждого фантома подряд
        public const float SwingTime = 0.16f;         // сам удар
        public const float OpenTime = 1.6f;           // сколько фантом открыт после блока
        public const float PauseMin = 0.8f;           // пауза перед следующим замахом
        public const float PauseMax = 2.0f;

        // Забег и очки.
        public const float RunSeconds = 180f;
        public const int ScoreKill = 100;
        public const int ScoreCleanSwing = 50;        // широкий быстрый взмах
        public const int ScoreWeakPoint = 100;        // попадание в светящееся ядро на груди
        public const int ScoreCounter = 50;           // удар сразу после блока щитом
        public const int ScoreParryCounter = 100;     // удар сразу после парирования мечом
        public const int ScoreBlock = 20;
        public const int ScoreParry = 50;
        public const float CleanSwingSpeed = 3.5f;    // м/с на кончике клинка
        public const float CleanSwingPath = 0.5f;     // м: сколько кончик прошёл до попадания
        public const float WeakPointRange = 0.11f;    // м: насколько близко клинок должен пройти к ядру
        public const float StreakStep = 0.1f;         // прибавка множителя за фантома без пропущенных ударов
        public const float StreakMax = 3f;
        public const int RankS = 6000, RankA = 4000, RankB = 2500, RankC = 1200;

        // Картинка и шлем.
        public const float TargetRefreshRate = 90f;
    }
}
