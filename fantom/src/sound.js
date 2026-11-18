/**
 * Звуки без файлов: всё синтезируется на лету через Web Audio.
 */
export class Sound {
    constructor() {
        this.ctx = null;
        this.noise = null;
    }

    ready() {
        try {
            this.ctx ??= new AudioContext();
            if (this.ctx.state === 'suspended') {
                this.ctx.resume();
            }
            if (!this.noise) {
                const length = this.ctx.sampleRate * 0.6;
                this.noise = this.ctx.createBuffer(1, length, this.ctx.sampleRate);
                const data = this.noise.getChannelData(0);
                for (let i = 0; i < length; i++) {
                    data[i] = Math.random() * 2 - 1;
                }
            }
            return true;
        } catch {
            return false; // звук не критичен
        }
    }

    tone(type, from, to, duration, volume, delay = 0) {
        const t = this.ctx.currentTime + delay;
        const osc = this.ctx.createOscillator();
        const gain = this.ctx.createGain();
        osc.type = type;
        osc.frequency.setValueAtTime(from, t);
        osc.frequency.exponentialRampToValueAtTime(Math.max(to, 1), t + duration);
        gain.gain.setValueAtTime(volume, t);
        gain.gain.exponentialRampToValueAtTime(0.0008, t + duration);
        osc.connect(gain).connect(this.ctx.destination);
        osc.start(t);
        osc.stop(t + duration + 0.02);
    }

    burst(filterType, frequency, duration, volume, delay = 0) {
        const t = this.ctx.currentTime + delay;
        const src = this.ctx.createBufferSource();
        src.buffer = this.noise;
        const filter = this.ctx.createBiquadFilter();
        filter.type = filterType;
        filter.frequency.value = frequency;
        const gain = this.ctx.createGain();
        gain.gain.setValueAtTime(volume, t);
        gain.gain.exponentialRampToValueAtTime(0.0008, t + duration);
        src.connect(filter).connect(gain).connect(this.ctx.destination);
        src.start(t);
        src.stop(t + duration + 0.02);
    }

    /** Фантом замахивается: нарастающий тон-предупреждение. */
    windup(duration) {
        if (!this.ready()) return;
        this.tone('sawtooth', 160, 520, duration, 0.05);
        this.tone('sine', 320, 1040, duration, 0.05);
    }

    /** Удар принят на щит. */
    block() {
        if (!this.ready()) return;
        this.burst('highpass', 2500, 0.12, 0.35);
        this.tone('triangle', 1250, 1180, 0.45, 0.16);
        this.tone('triangle', 1870, 1790, 0.35, 0.1);
        this.tone('sine', 2960, 2900, 0.25, 0.06);
    }

    /** Меч попал по фантому: удар и «стеклянное» рассыпание. */
    shatter() {
        if (!this.ready()) return;
        this.tone('sine', 190, 45, 0.22, 0.5);
        this.burst('bandpass', 1800, 0.18, 0.4);
        const notes = [1568, 2093, 2637, 3136, 2349, 3520, 4186, 2794];
        for (let i = 0; i < 14; i++) {
            const note = notes[Math.floor(Math.random() * notes.length)];
            this.tone('sine', note, note * 0.985, 0.2, 0.05, 0.03 + i * 0.045 + Math.random() * 0.03);
        }
        this.burst('highpass', 6000, 0.7, 0.07, 0.05);
    }

    /** Меч ударил, но фантом ещё не открыт. */
    deflect() {
        if (!this.ready()) return;
        this.tone('square', 700, 500, 0.06, 0.05);
    }

    /** Игрок пропустил удар. */
    hurt() {
        if (!this.ready()) return;
        this.tone('sawtooth', 130, 40, 0.4, 0.3);
        this.burst('lowpass', 500, 0.3, 0.5);
    }

    /** Появился новый фантом. */
    spawn() {
        if (!this.ready()) return;
        this.tone('sine', 220, 660, 0.7, 0.06);
        this.tone('sine', 330, 990, 0.7, 0.04, 0.08);
    }
}
