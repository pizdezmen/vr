/**
 * Бой с фантомом.
 * Круг игры: фантом замахивается (клинок раскаляется) -> игрок подставляет щит
 * -> фантом «раскрывается» -> игрок бьёт мечом -> фантом рассыпается на пиксели.
 * Замеряем: время реакции на замах и время ответного удара.
 */
import {
    AdditiveBlending,
    BackSide,
    BoxGeometry,
    CanvasTexture,
    CylinderGeometry,
    createSystem,
    DoubleSide,
    Group,
    Mesh,
    MeshBasicMaterial,
    MeshStandardMaterial,
    PlaneGeometry,
    RingGeometry,
    SphereGeometry,
    Vector3,
    VisibilityState,
} from '@iwsdk/core';
import { buildArena } from './arena.js';
import { ATTACKS, BODY_CAPSULES, Phantom, POSES } from './phantom.js';
import { Sound } from './sound.js';

// ---- Настройки ощущений. Их удобно крутить после пробы в шлеме. ----
const SWORD_HAND = 'right';
const SHIELD_HAND = 'left';
const PLAYER_BLADE = 0.85; // длина меча игрока, м
const BLADE_TILT = -0.15; // наклон клинка от оси рукоятки контроллера, радианы (минус = вперёд)
const SHIELD_RADIUS = 0.27;
const BLOCK_RANGE = 0.25; // щит ближе этого к клинку фантома = блок
const PARRY_RANGE = 0.09; // меч игрока ближе этого к клинку фантома = парирование
const FIGHT_DISTANCE = 1.25; // на каком расстоянии стоит фантом
const MIN_SWING_SPEED = 1.3; // м/с: медленное касание мечом не считается ударом
const WINDUP_START = 0.95; // секунд на замах в начале
const WINDUP_MIN = 0.38; // быстрее этого замах не станет
const WINDUP_STEP = 0.05; // на сколько ускоряется замах за каждого фантома подряд
const SWING_TIME = 0.16; // сам удар фантома
const OPEN_TIME = 1.6; // сколько фантом открыт после блока
const PAUSE_MIN = 0.8; // пауза перед следующим замахом: от и до
const PAUSE_MAX = 2.0;

/** Ближайшие точки двух отрезков (p1-q1 и p2-q2). Возвращает расстояние. */
function segmentDistance(p1, q1, p2, q2, out1, out2, d1, d2, r) {
    d1.subVectors(q1, p1);
    d2.subVectors(q2, p2);
    r.subVectors(p1, p2);
    const a = d1.dot(d1);
    const e = d2.dot(d2);
    const f = d2.dot(r);
    let s = 0;
    let t = 0;
    if (a > 1e-8 && e > 1e-8) {
        const b = d1.dot(d2);
        const c = d1.dot(r);
        const denom = a * e - b * b;
        s = denom > 1e-8 ? Math.min(Math.max((b * f - c * e) / denom, 0), 1) : 0;
        t = (b * s + f) / e;
        if (t < 0) {
            t = 0;
            s = Math.min(Math.max(-c / a, 0), 1);
        } else if (t > 1) {
            t = 1;
            s = Math.min(Math.max((b - c) / a, 0), 1);
        }
    } else if (e > 1e-8) {
        t = Math.min(Math.max(f / e, 0), 1);
    } else if (a > 1e-8) {
        s = Math.min(Math.max(-d1.dot(r) / a, 0), 1);
    }
    out1.copy(p1).addScaledVector(d1, s);
    out2.copy(p2).addScaledVector(d2, t);
    return out1.distanceTo(out2);
}

export class FightSystem extends createSystem({}) {
    init() {
        if (import.meta.env.DEV) {
            globalThis.__fantom = this; // только для проверки в эмуляторе
        }
        this.sound = new Sound();
        this.time = 0;

        // Рабочие векторы, чтобы не создавать новые каждый кадр.
        this.headPos = new Vector3();
        this.headDir = new Vector3();
        this.swordBase = new Vector3();
        this.swordTip = new Vector3();
        this.prevTip = new Vector3();
        this.tipVelocity = new Vector3();
        this.shieldPos = new Vector3();
        this.dangerBase = new Vector3();
        this.dangerTip = new Vector3();
        this.c1 = new Vector3();
        this.c2 = new Vector3();
        this.t1 = new Vector3();
        this.t2 = new Vector3();
        this.t3 = new Vector3();
        this.localBase = new Vector3();
        this.localTip = new Vector3();
        this.hitPoint = new Vector3();
        this.push = new Vector3();
        this.goal = new Vector3();

        // Мир вокруг.
        this.arena = buildArena();
        this.world.createTransformEntity(this.arena.group);

        // Фантом.
        this.phantom = new Phantom();
        this.phantom.hide();
        this.world.createTransformEntity(this.phantom.root);

        this.buildSword();
        this.buildShield();
        this.buildBoard();

        // Красная вспышка, когда игрок пропустил удар.
        this.hurtFlash = new Mesh(
            new SphereGeometry(0.35, 16, 12),
            new MeshBasicMaterial({
                color: 0xff2030,
                transparent: true,
                opacity: 0,
                side: BackSide,
                depthTest: false,
                depthWrite: false,
            }),
        );
        this.hurtFlash.renderOrder = 50;
        this.hurtFlash.visible = false;
        this.world.createTransformEntity(this.hurtFlash);

        this.resetGame();
        this.cleanupFuncs.push(
            this.world.visibilityState.subscribe((state) => {
                if (state === VisibilityState.NonImmersive) {
                    this.resetGame();
                }
            }),
        );
    }

    buildSword() {
        const sword = new Group();
        const steel = new MeshStandardMaterial({ color: 0x20304a, roughness: 0.4, metalness: 0.6 });
        const handle = new Mesh(new CylinderGeometry(0.016, 0.018, 0.16, 10), steel);
        handle.rotation.x = Math.PI / 2;
        handle.position.z = 0.02;
        const guard = new Mesh(new BoxGeometry(0.16, 0.022, 0.022), steel);
        guard.position.z = -0.07;
        const blade = new Mesh(
            new BoxGeometry(0.034, 0.008, PLAYER_BLADE),
            new MeshBasicMaterial({ color: 0xa8d8ff }),
        );
        blade.position.z = -0.08 - PLAYER_BLADE / 2;
        const glow = new Mesh(
            new BoxGeometry(0.06, 0.02, PLAYER_BLADE + 0.03),
            new MeshBasicMaterial({
                color: 0x1c55d0,
                transparent: true,
                opacity: 0.3,
                blending: AdditiveBlending,
                depthWrite: false,
            }),
        );
        glow.position.copy(blade.position);
        sword.add(handle, guard, blade, glow);
        sword.rotation.x = BLADE_TILT;
        // Точки основания и кончика клинка: по ним считаем попадания.
        this.swordBaseMark = new Group();
        this.swordBaseMark.position.z = -0.1;
        this.swordTipMark = new Group();
        this.swordTipMark.position.z = -0.08 - PLAYER_BLADE;
        sword.add(this.swordBaseMark, this.swordTipMark);
        sword.visible = false;
        this.sword = sword;
        this.player.gripSpaces[SWORD_HAND].add(sword);
    }

    buildShield() {
        const shield = new Group();
        this.shieldMaterial = new MeshBasicMaterial({
            color: 0x2a78ff,
            transparent: true,
            opacity: 0.14,
            blending: AdditiveBlending,
            depthWrite: false,
            side: DoubleSide,
        });
        const disc = new Mesh(new CylinderGeometry(SHIELD_RADIUS, SHIELD_RADIUS, 0.012, 6), this.shieldMaterial);
        disc.rotation.x = Math.PI / 2;
        const rim = new Mesh(
            new RingGeometry(SHIELD_RADIUS - 0.012, SHIELD_RADIUS, 6),
            new MeshBasicMaterial({
                color: 0x6fb4ff,
                transparent: true,
                opacity: 0.7,
                blending: AdditiveBlending,
                depthWrite: false,
                side: DoubleSide,
            }),
        );
        rim.rotation.z = Math.PI / 6;
        const core = new Mesh(new RingGeometry(0.05, 0.058, 6), rim.material);
        core.rotation.z = Math.PI / 6;
        shield.add(disc, rim, core);
        shield.position.set(0, 0, -0.1);
        shield.visible = false;
        this.shield = shield;
        this.shieldFlash = 0;
        // Щит смотрит туда же, куда указывает контроллер.
        this.player.raySpaces[SHIELD_HAND].add(shield);
    }

    buildBoard() {
        this.boardCanvas = document.createElement('canvas');
        this.boardCanvas.width = 1024;
        this.boardCanvas.height = 640;
        this.boardTexture = new CanvasTexture(this.boardCanvas);
        this.board = new Mesh(
            new PlaneGeometry(2.4, 1.5),
            new MeshBasicMaterial({ map: this.boardTexture, transparent: true, depthWrite: false }),
        );
        this.board.visible = false;
        this.world.createTransformEntity(this.board);
    }

    resetGame() {
        this.state = 'waiting'; // ждём, пока шлем отдаст позицию головы
        this.stateTime = 0;
        this.placed = false;
        this.streak = 0;
        this.kills = 0;
        this.blocks = 0;
        this.misses = 0;
        this.reactions = [];
        this.lastReaction = null;
        this.lastCounter = null;
        this.bestReaction = null;
        this.message = 'Щит в левой руке, меч в правой';
        this.guardSince = null;
        this.deflectCooldown = 0;
        this.hasPrevTip = false;
        this.attack = ATTACKS[0];
        this.phantom?.hide();
        if (this.board) this.board.visible = false;
        if (this.sword) this.sword.visible = false;
        if (this.shield) this.shield.visible = false;
        if (this.hurtFlash) this.hurtFlash.visible = false;
        if (this.boardCanvas) this.drawBoard();
    }

    get windupTime() {
        return Math.max(WINDUP_MIN, WINDUP_START - WINDUP_STEP * this.streak);
    }

    drawBoard() {
        const ctx = this.boardCanvas.getContext('2d');
        const w = 1024;
        ctx.clearRect(0, 0, w, 640);
        ctx.fillStyle = 'rgba(4, 12, 34, 0.72)';
        ctx.beginPath();
        ctx.roundRect(4, 4, w - 8, 632, 36);
        ctx.fill();
        ctx.strokeStyle = 'rgba(80, 160, 255, 0.9)';
        ctx.lineWidth = 5;
        ctx.stroke();

        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillStyle = '#7fb8ff';
        ctx.font = '600 40px sans-serif';
        ctx.fillText('РЕАКЦИЯ НА УДАР', w / 2, 62);

        ctx.fillStyle = '#ffffff';
        ctx.font = 'bold 170px sans-serif';
        ctx.fillText(this.lastReaction == null ? '—' : `${this.lastReaction} мс`, w / 2, 190);

        ctx.font = '600 46px sans-serif';
        ctx.fillStyle = '#ffd27a';
        ctx.fillText(this.message, w / 2, 318);

        ctx.font = '44px sans-serif';
        ctx.fillStyle = '#cfe4ff';
        const average = this.reactions.length
            ? Math.round(this.reactions.reduce((sum, v) => sum + v, 0) / this.reactions.length)
            : null;
        const ms = (v) => (v == null ? '—' : `${v} мс`);
        ctx.fillText(`Средняя: ${ms(average)}     Лучшая: ${ms(this.bestReaction)}`, w / 2, 400);
        ctx.fillText(`Ответный удар: ${ms(this.lastCounter)}`, w / 2, 468);

        ctx.font = '40px sans-serif';
        ctx.fillStyle = '#8fb3e8';
        ctx.fillText(
            `Фантомов: ${this.kills}    Блоков: ${this.blocks}    Пропущено: ${this.misses}`,
            w / 2, 545,
        );
        ctx.fillText(`Замах фантома: ${Math.round(this.windupTime * 1000)} мс`, w / 2, 598);
        this.boardTexture.needsUpdate = true;
    }

    say(message) {
        this.message = message;
        this.drawBoard();
    }

    pulse(hand, strength, ms) {
        this.input.xr.gamepads[hand]?.gamepad?.hapticActuators?.[0]?.pulse?.(strength, ms);
    }

    setState(state) {
        this.state = state;
        this.stateTime = 0;
    }

    /** Ставим фантома и табло перед игроком в начале боя. */
    placeArena() {
        this.headDir.y = 0;
        if (this.headDir.lengthSq() < 0.0001) this.headDir.set(0, 0, -1);
        this.headDir.normalize();
        const scale = Math.min(Math.max(this.headPos.y / 1.62, 0.6), 1.25);
        const root = this.phantom.root;
        root.scale.setScalar(scale);
        root.position.copy(this.headPos).addScaledVector(this.headDir, FIGHT_DISTANCE * scale);
        root.position.y = 0;
        this.facePlayer(1);

        this.board.position.copy(this.headPos).addScaledVector(this.headDir, 4.2);
        this.board.position.y = this.headPos.y + 1.35;
        this.board.lookAt(this.headPos);
        this.board.visible = true;
        this.sword.visible = true;
        this.shield.visible = true;
        this.placed = true;
    }

    /** Фантом поворачивается к игроку и держит дистанцию. */
    facePlayer(k) {
        const root = this.phantom.root;
        this.goal.copy(root.position).sub(this.headPos);
        this.goal.y = 0;
        if (this.goal.lengthSq() < 0.01) return;
        this.goal.normalize().multiplyScalar(FIGHT_DISTANCE * root.scale.x).add(this.headPos);
        this.goal.y = 0;
        root.position.lerp(this.goal, k);
        root.lookAt(this.headPos.x, 0, this.headPos.z);
    }

    spawnPhantom() {
        this.facePlayer(1);
        this.phantom.startAssemble();
        this.sound.spawn();
        this.setState('assemble');
    }

    /** Защищён ли игрок от удара, который закончится в позе attack.hit. */
    isGuarding() {
        if (this.shieldPos.distanceTo(this.closestOnDanger(this.shieldPos)) < BLOCK_RANGE) {
            return SHIELD_HAND;
        }
        const d = segmentDistance(
            this.swordBase, this.swordTip, this.dangerBase, this.dangerTip,
            this.c1, this.c2, this.t1, this.t2, this.t3,
        );
        return d < PARRY_RANGE ? SWORD_HAND : null;
    }

    closestOnDanger(point) {
        this.t1.subVectors(this.dangerTip, this.dangerBase);
        const k = Math.min(Math.max(this.t2.subVectors(point, this.dangerBase).dot(this.t1) / this.t1.lengthSq(), 0), 1);
        return this.c1.copy(this.dangerBase).addScaledVector(this.t1, k);
    }

    startAttack() {
        // Выбираем удар, от которого игрок сейчас НЕ закрыт: стоять со щитом на месте не поможет.
        const start = Math.floor(Math.random() * ATTACKS.length);
        for (let i = 0; i < ATTACKS.length; i++) {
            this.attack = ATTACKS[(start + i) % ATTACKS.length];
            this.phantom.bladeSegment(this.attack.hit, this.dangerBase, this.dangerTip);
            if (!this.isGuarding()) break;
        }
        this.guardSince = null;
        this.currentWindup = this.windupTime;
        this.sound.windup(this.currentWindup + SWING_TIME);
        this.setState('windup');
    }

    resolveImpact() {
        const guardHand = this.isGuarding();
        if (guardHand) {
            this.blocks += 1;
            const reaction = Math.round((this.guardSince ?? this.stateTime + this.currentWindup) * 1000);
            this.lastReaction = reaction;
            this.reactions.push(reaction);
            this.bestReaction = this.bestReaction == null ? reaction : Math.min(this.bestReaction, reaction);
            this.sound.block();
            this.pulse(guardHand, 1, 90);
            this.shieldFlash = guardHand === SHIELD_HAND ? 1 : 0;
            this.say(guardHand === SHIELD_HAND ? 'Блок! Бей мечом!' : 'Парирование! Бей!');
            this.setState('open');
        } else {
            this.misses += 1;
            this.streak = 0;
            this.sound.hurt();
            this.pulse('left', 1, 200);
            this.pulse('right', 1, 200);
            this.hurtFlash.material.opacity = 0.55;
            this.hurtFlash.visible = true;
            this.say('Пропустил удар');
            this.setState('recover');
        }
    }

    /** Проверяем, задел ли меч игрока тело фантома. */
    checkSwordHit() {
        const root = this.phantom.root;
        this.localBase.copy(this.swordBase);
        this.localTip.copy(this.swordTip);
        root.worldToLocal(this.localBase);
        root.worldToLocal(this.localTip);
        for (const capsule of BODY_CAPSULES) {
            const d = segmentDistance(
                this.localBase, this.localTip, capsule.a, capsule.b,
                this.c1, this.c2, this.t1, this.t2, this.t3,
            );
            if (d < capsule.r) {
                this.hitPoint.copy(this.c1);
                return true;
            }
        }
        return false;
    }

    killPhantom() {
        const root = this.phantom.root;
        // Скорость меча переводим в координаты фантома: туда и полетят пиксели.
        this.push.copy(this.tipVelocity).add(root.position);
        root.worldToLocal(this.push);
        this.t1.copy(root.position);
        root.worldToLocal(this.t1);
        this.push.sub(this.t1).clampLength(0, 6);
        this.phantom.startDissolve(this.hitPoint, this.push);
        this.kills += 1;
        this.streak += 1;
        this.lastCounter = Math.round(this.stateTime * 1000);
        this.sound.shatter();
        this.pulse(SWORD_HAND, 1, 140);
        this.say('Фантом рассыпался!');
        this.setState('dissolve');
    }

    update(delta) {
        const immersive = this.world.visibilityState.peek() !== VisibilityState.NonImmersive;
        const dt = Math.min(delta, 0.05);
        this.time += dt;
        this.arena.update(this.time);
        if (!immersive) return;

        const head = this.player.head;
        head.updateWorldMatrix(true, false);
        this.headPos.setFromMatrixPosition(head.matrixWorld);
        if (this.headPos.y < 0.3) return; // шлем ещё не отдаёт позицию головы
        if (!this.placed) {
            head.getWorldDirection(this.headDir);
            this.headDir.negate(); // камера смотрит вдоль -Z
            this.placeArena();
            this.spawnPhantom();
        }

        // В бою стандартные модели контроллеров не нужны: в руках меч и щит.
        const adapters = this.input.xr.visualAdapters;
        for (const hand of ['left', 'right']) {
            const model = adapters?.[hand]?.peek?.()?.visual?.model;
            if (model) model.visible = false;
        }

        // Где сейчас меч и щит.
        this.swordBaseMark.getWorldPosition(this.swordBase);
        this.swordTipMark.getWorldPosition(this.swordTip);
        this.shield.getWorldPosition(this.shieldPos);
        if (this.hasPrevTip && dt > 0) {
            this.t1.subVectors(this.swordTip, this.prevTip).divideScalar(dt);
            this.tipVelocity.lerp(this.t1, 0.6);
        }
        this.prevTip.copy(this.swordTip);
        this.hasPrevTip = true;
        const swordSpeed = this.tipVelocity.length();

        // Затухание вспышек.
        if (this.hurtFlash.visible) {
            this.hurtFlash.position.copy(this.headPos);
            this.hurtFlash.material.opacity -= dt * 1.4;
            if (this.hurtFlash.material.opacity <= 0) this.hurtFlash.visible = false;
        }
        this.shieldFlash = Math.max(0, this.shieldFlash - dt * 4);
        this.shieldMaterial.opacity = 0.14 + this.shieldFlash * 0.7;
        this.shield.scale.setScalar(1 + this.shieldFlash * 0.12);
        this.deflectCooldown -= dt;

        const phantom = this.phantom;
        this.stateTime += dt;
        const t = this.stateTime;
        const bob = Math.sin(this.time * 2.2) * 0.012;

        switch (this.state) {
            case 'assemble':
                if (phantom.updateFx(dt)) {
                    this.pause = PAUSE_MIN + Math.random() * (PAUSE_MAX - PAUSE_MIN);
                    this.setState('idle');
                }
                break;

            case 'idle':
                this.facePlayer(Math.min(dt * 3, 1));
                phantom.root.position.y = bob;
                phantom.setPose(POSES.idle, POSES.idle, 0);
                if (t >= this.pause) this.startAttack();
                break;

            case 'windup': {
                // Замах: рука уходит назад, клинок раскаляется. Это и есть подсказка игроку.
                const k = Math.min(t / (this.currentWindup * 0.55), 1);
                phantom.setPose(POSES.idle, this.attack.wind, 1 - (1 - k) * (1 - k));
                phantom.setHeat(Math.min(t / this.currentWindup, 1));
                this.trackGuard(t);
                if (t >= this.currentWindup) this.setState('swing');
                break;
            }

            case 'swing': {
                const k = Math.min(t / SWING_TIME, 1);
                phantom.setPose(this.attack.wind, this.attack.hit, k * k);
                this.trackGuard(this.currentWindup + t);
                if (k >= 1) this.resolveImpact();
                break;
            }

            case 'open': {
                // Фантом отброшен и открыт: мерцает белым, по нему можно бить.
                const k = Math.min(t / 0.18, 1);
                phantom.setPose(this.attack.hit, POSES.open, 1 - (1 - k) * (1 - k));
                phantom.setHeat(Math.max(0, 1 - t * 4));
                phantom.setOpen(1, this.time);
                if (swordSpeed > MIN_SWING_SPEED && this.checkSwordHit()) {
                    this.killPhantom();
                } else if (t >= OPEN_TIME) {
                    this.say('Не успел ударить');
                    this.setState('recover');
                }
                break;
            }

            case 'recover': {
                const k = Math.min(t / 0.6, 1);
                phantom.setPose(phantom.openness > 0 ? POSES.open : this.attack.hit, POSES.idle, k);
                phantom.setHeat(Math.max(0, phantom.heat - dt * 3));
                if (k >= 1) {
                    phantom.setOpen(0, this.time);
                    phantom.setHeat(0);
                    this.pause = PAUSE_MIN + Math.random() * (PAUSE_MAX - PAUSE_MIN);
                    this.setState('idle');
                }
                break;
            }

            case 'dissolve':
                if (phantom.updateFx(dt) && t > 1.3) {
                    this.spawnPhantom();
                }
                break;
        }

        // Удар мечом по закрытому фантому: лёгкий отскок, чтобы было понятно, что рано.
        if (
            (this.state === 'idle' || this.state === 'windup') &&
            this.deflectCooldown <= 0 && swordSpeed > MIN_SWING_SPEED && this.checkSwordHit()
        ) {
            this.deflectCooldown = 0.5;
            this.sound.deflect();
            this.pulse(SWORD_HAND, 0.3, 30);
            if (this.message !== 'Сначала блок щитом') this.say('Сначала блок щитом');
        }
    }

    /** Запоминаем момент, когда игрок встал в защиту (от начала замаха). */
    trackGuard(sinceWindup) {
        if (this.isGuarding()) {
            this.guardSince ??= sinceWindup;
        } else {
            this.guardSince = null;
        }
    }
}
