/**
 * Фантом: фигура воина, собранная из светящихся синих кубиков («пикселей»).
 * Умеет: стойка, замах, удар, «раскрыться» после блока,
 * собраться из пикселей и рассыпаться на пиксели от точки попадания.
 *
 * Локальные оси фантома: +Z смотрит на игрока, +X это право игрока, Y вверх.
 */
import {
    AdditiveBlending,
    BoxGeometry,
    Color,
    Group,
    InstancedMesh,
    Matrix4,
    MeshBasicMaterial,
    Quaternion,
    Vector3,
} from '@iwsdk/core';

const CELL = 0.036; // размер одного «пикселя», м
const CUBE = CELL * 0.8;
export const ARM_LENGTH = 0.55;
export const BLADE_LENGTH = 0.9;
export const SHOULDER = new Vector3(-0.26, 1.42, 0);

const Z_AXIS = new Vector3(0, 0, 1);

/** Позы руки с мечом: направление руки от плеча и направление клинка. */
export const POSES = {
    idle: { arm: [-0.25, -0.75, 0.6], blade: [0.25, 0.9, 0.35] },
    open: { arm: [-0.95, 0.1, -0.3], blade: [-0.7, -0.5, -0.5] },
    // Удар сверху по голове.
    topWind: { arm: [-0.1, 0.9, -0.3], blade: [0, 0.6, -0.8] },
    topHit: { arm: [0.3, 0.5, 0.8], blade: [0.1, 0.03, 1] },
    // Удар сбоку: приходит слева от игрока.
    leftWind: { arm: [-0.9, 0.25, -0.2], blade: [-0.6, 0.5, -0.6] },
    leftHit: { arm: [-0.35, -0.2, 0.9], blade: [-0.055, -0.23, 0.97] },
    // Удар наотмашь: приходит справа от игрока.
    rightWind: { arm: [0.75, 0.45, 0.35], blade: [0.7, 0.6, -0.4] },
    rightHit: { arm: [0.55, -0.2, 0.85], blade: [0.467, -0.193, 0.847] },
};
for (const pose of Object.values(POSES)) {
    pose.arm = new Vector3(...pose.arm).normalize();
    pose.blade = new Vector3(...pose.blade).normalize();
}

export const ATTACKS = [
    { name: 'top', wind: POSES.topWind, hit: POSES.topHit },
    { name: 'left', wind: POSES.leftWind, hit: POSES.leftHit },
    { name: 'right', wind: POSES.rightWind, hit: POSES.rightHit },
];

/** Упрощённое тело для проверки попадания мечом: отрезки с радиусом. */
export const BODY_CAPSULES = [
    { a: new Vector3(0, 1.6, 0), b: new Vector3(0, 1.64, 0), r: 0.17 },
    { a: new Vector3(0, 0.95, 0), b: new Vector3(0, 1.4, 0), r: 0.25 },
    { a: new Vector3(0, 0.15, 0), b: new Vector3(0, 0.85, 0), r: 0.21 },
];

function ellipsoid(cx, cy, cz, rx, ry, rz) {
    return (x, y, z) => ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 + ((z - cz) / rz) ** 2 <= 1;
}

/** Оставляем только «кожу» фигуры: кубики на поверхности. */
function voxelize(shapes, min, max) {
    const inside = (x, y, z) => shapes.some((shape) => shape(x, y, z));
    const out = [];
    for (let x = min[0]; x <= max[0]; x += CELL) {
        for (let y = min[1]; y <= max[1]; y += CELL) {
            for (let z = min[2]; z <= max[2]; z += CELL) {
                if (!inside(x, y, z)) continue;
                const surface =
                    !inside(x + CELL, y, z) || !inside(x - CELL, y, z) ||
                    !inside(x, y + CELL, z) || !inside(x, y - CELL, z) ||
                    !inside(x, y, z + CELL) || !inside(x, y, z - CELL);
                if (surface) out.push(x, y, z);
            }
        }
    }
    return out;
}

function makeMaterial(color) {
    return new MeshBasicMaterial({
        color,
        transparent: true,
        blending: AdditiveBlending,
        depthWrite: false,
    });
}

const BODY_COLOR = new Color(0.03, 0.16, 0.5);
const OPEN_COLOR = new Color(0.45, 0.6, 0.7);
const BLADE_COLD = new Color(0.12, 0.35, 0.8);
const BLADE_HOT = new Color(1.0, 0.22, 0.04);

export class Phantom {
    constructor() {
        this.root = new Group();
        this.geometry = new BoxGeometry(CUBE, CUBE, CUBE);

        const body = voxelize(
            [
                ellipsoid(0, 1.62, 0, 0.115, 0.135, 0.125), // голова
                ellipsoid(0, 1.2, 0, 0.215, 0.29, 0.125), // торс
                ellipsoid(0, 0.9, 0, 0.175, 0.15, 0.115), // таз
                ellipsoid(0.1, 0.44, 0, 0.085, 0.45, 0.09), // ноги
                ellipsoid(-0.1, 0.44, 0, 0.085, 0.45, 0.09),
                ellipsoid(0.3, 1.14, 0.02, 0.06, 0.31, 0.065), // левая рука
            ],
            [-0.4, 0, -0.16],
            [0.4, 1.8, 0.16],
        );
        const arm = voxelize(
            [ellipsoid(0, 0, ARM_LENGTH / 2, 0.06, 0.06, ARM_LENGTH / 2 + 0.03)],
            [-CELL * 2, -CELL * 2, -CELL],
            [CELL * 2, CELL * 2, ARM_LENGTH + 0.05],
        );
        const blade = [];
        for (let z = -0.06; z <= BLADE_LENGTH; z += CELL * 0.75) {
            blade.push(0, 0, z);
            if (z > 0.06) blade.push(0, CELL * 0.7, z);
        }
        for (let x = -0.1; x <= 0.1; x += CELL) blade.push(x, 0, 0.06); // гарда

        this.bodyMaterial = makeMaterial(BODY_COLOR);
        this.bladeMaterial = makeMaterial(BLADE_COLD);
        this.bodyMesh = this.makePart(body, this.bodyMaterial);
        this.armMesh = this.makePart(arm, this.bodyMaterial);
        this.bladeMesh = this.makePart(blade, this.bladeMaterial);
        this.armMesh.position.copy(SHOULDER);
        this.root.add(this.bodyMesh, this.armMesh, this.bladeMesh);

        // Все кубики ещё раз, одним набором: для сборки и рассыпания.
        this.counts = [body.length / 3, arm.length / 3, blade.length / 3];
        this.total = this.counts[0] + this.counts[1] + this.counts[2];
        this.partPoints = [body, arm, blade];
        this.fxMesh = new InstancedMesh(this.geometry, makeMaterial(0xffffff), this.total);
        this.fxMesh.frustumCulled = false;
        this.fxMesh.visible = false;
        const white = new Color(1, 1, 1);
        for (let i = 0; i < this.total; i++) this.fxMesh.setColorAt(i, white);
        this.root.add(this.fxMesh);
        this.home = new Float32Array(this.total * 3);
        this.velocity = new Float32Array(this.total * 3);
        this.delay = new Float32Array(this.total);
        this.shade = new Float32Array(this.total);
        this.isBlade = new Uint8Array(this.total);
        for (let i = 0; i < this.total; i++) {
            this.shade[i] = 0.55 + Math.random() * 0.45;
            this.isBlade[i] = i >= this.counts[0] + this.counts[1] ? 1 : 0;
        }

        this.fx = null; // 'assemble' | 'dissolve' | null
        this.fxTime = 0;
        this.fxDuration = 0;
        this.arm = new Vector3();
        this.blade = new Vector3();
        this.hand = new Vector3();
        this.heat = 0;
        this.openness = 0;
        this.tmp = new Vector3();
        this.quat = new Quaternion();
        this.setPose(POSES.idle, POSES.idle, 0);
    }

    makePart(points, material) {
        const count = points.length / 3;
        const mesh = new InstancedMesh(this.geometry, material, count);
        const matrix = new Matrix4();
        const color = new Color();
        for (let i = 0; i < count; i++) {
            matrix.makeTranslation(points[i * 3], points[i * 3 + 1], points[i * 3 + 2]);
            mesh.setMatrixAt(i, matrix);
            mesh.setColorAt(i, color.setScalar(0.55 + Math.random() * 0.45));
        }
        mesh.frustumCulled = false;
        return mesh;
    }

    /** Поза руки: смесь двух поз, k от 0 до 1. */
    setPose(from, to, k) {
        this.arm.copy(from.arm).lerp(to.arm, k);
        if (this.arm.lengthSq() < 0.01) this.arm.set(0, 1, 0);
        this.arm.normalize();
        this.blade.copy(from.blade).lerp(to.blade, k);
        if (this.blade.lengthSq() < 0.01) this.blade.set(0, 1, 0);
        this.blade.normalize();
        this.hand.copy(SHOULDER).addScaledVector(this.arm, ARM_LENGTH);
        this.armMesh.quaternion.setFromUnitVectors(Z_AXIS, this.arm);
        this.bladeMesh.position.copy(this.hand);
        this.bladeMesh.quaternion.setFromUnitVectors(Z_AXIS, this.blade);
    }

    /** Концы клинка для заданной позы в мировых координатах. */
    bladeSegment(pose, outBase, outTip) {
        outBase.copy(SHOULDER).addScaledVector(pose.arm, ARM_LENGTH);
        outTip.copy(outBase).addScaledVector(pose.blade, BLADE_LENGTH);
        outBase.addScaledVector(pose.blade, BLADE_LENGTH * 0.45); // опасна дальняя часть клинка
        this.root.updateWorldMatrix(true, false);
        outBase.applyMatrix4(this.root.matrixWorld);
        outTip.applyMatrix4(this.root.matrixWorld);
    }

    /** heat: 0 холодный клинок, 1 раскалённый (сейчас ударит). */
    setHeat(heat) {
        this.heat = heat;
        this.bladeMaterial.color.copy(BLADE_COLD).lerp(BLADE_HOT, heat);
    }

    /** openness: 0 обычный, 1 раскрыт для ответного удара (мерцает белым). */
    setOpen(openness, time) {
        this.openness = openness;
        const flicker = openness > 0 ? 0.6 + 0.4 * Math.sin(time * 28) : 0;
        this.bodyMaterial.color.copy(BODY_COLOR).lerp(OPEN_COLOR, openness * flicker);
    }

    setPartsVisible(visible) {
        this.bodyMesh.visible = visible;
        this.armMesh.visible = visible;
        this.bladeMesh.visible = visible;
    }

    /** Запоминаем, где сейчас стоит каждый кубик (в координатах фантома). */
    captureHome() {
        const parts = [this.bodyMesh, this.armMesh, this.bladeMesh];
        let index = 0;
        for (let p = 0; p < 3; p++) {
            const points = this.partPoints[p];
            parts[p].updateMatrix();
            for (let i = 0; i < this.counts[p]; i++, index++) {
                this.tmp.set(points[i * 3], points[i * 3 + 1], points[i * 3 + 2]).applyMatrix4(parts[p].matrix);
                this.home[index * 3] = this.tmp.x;
                this.home[index * 3 + 1] = this.tmp.y;
                this.home[index * 3 + 2] = this.tmp.z;
            }
        }
    }

    /** Фантом собирается из пикселей снизу вверх. */
    startAssemble() {
        this.setHeat(0);
        this.setOpen(0, 0);
        this.setPose(POSES.idle, POSES.idle, 0);
        this.captureHome();
        for (let i = 0; i < this.total; i++) {
            this.delay[i] = (this.home[i * 3 + 1] / 1.8) * 0.55 + Math.random() * 0.1;
            const angle = Math.random() * Math.PI * 2;
            const radius = 0.3 + Math.random() * 0.7;
            this.velocity[i * 3] = Math.cos(angle) * radius;
            this.velocity[i * 3 + 1] = -0.2 - Math.random() * 0.5;
            this.velocity[i * 3 + 2] = Math.sin(angle) * radius;
        }
        this.fx = 'assemble';
        this.fxTime = 0;
        this.fxDuration = 1.05;
        this.setPartsVisible(false);
        this.fxMesh.visible = true;
        this.updateFx(0);
    }

    /**
     * Фантом рассыпается. hit: точка попадания, push: скорость меча
     * (обе в координатах фантома).
     */
    startDissolve(hit, push) {
        this.captureHome();
        const waveSpeed = 2.6; // м/с: как быстро распад бежит по телу от точки удара
        let maxDelay = 0;
        for (let i = 0; i < this.total; i++) {
            const dx = this.home[i * 3] - hit.x;
            const dy = this.home[i * 3 + 1] - hit.y;
            const dz = this.home[i * 3 + 2] - hit.z;
            const dist = Math.sqrt(dx * dx + dy * dy + dz * dz) + 0.001;
            this.delay[i] = dist / waveSpeed + Math.random() * 0.04;
            maxDelay = Math.max(maxDelay, this.delay[i]);
            const near = Math.max(0, 1 - dist / 0.5); // у места удара разлёт сильнее
            const out = 0.35 + Math.random() * 0.6 + near * 1.6;
            const carry = (0.12 + near * 0.35) * Math.random();
            this.velocity[i * 3] = (dx / dist) * out + push.x * carry + (Math.random() - 0.5) * 0.3;
            this.velocity[i * 3 + 1] = (dy / dist) * out + push.y * carry + 0.25 + Math.random() * 0.5;
            this.velocity[i * 3 + 2] = (dz / dist) * out + push.z * carry + (Math.random() - 0.5) * 0.3;
        }
        this.fx = 'dissolve';
        this.fxTime = 0;
        this.fxDuration = maxDelay + DISSOLVE_LIFE;
        this.setPartsVisible(false);
        this.fxMesh.visible = true;
        this.updateFx(0);
    }

    /** Возвращает true, когда эффект закончился. */
    updateFx(dt) {
        if (!this.fx) return true;
        this.fxTime += dt;
        const matrices = this.fxMesh.instanceMatrix.array;
        const colors = this.fxMesh.instanceColor.array;
        const assemble = this.fx === 'assemble';
        for (let i = 0; i < this.total; i++) {
            const t = this.fxTime - this.delay[i];
            let x = this.home[i * 3];
            let y = this.home[i * 3 + 1];
            let z = this.home[i * 3 + 2];
            let scale = 1;
            let r = BODY_COLOR.r, g = BODY_COLOR.g, b = BODY_COLOR.b;
            if (this.isBlade[i]) {
                r = this.bladeMaterial.color.r; g = this.bladeMaterial.color.g; b = this.bladeMaterial.color.b;
            }
            let bright = this.shade[i];
            if (assemble) {
                const k = Math.min(Math.max(t / 0.45, 0), 1);
                const rest = (1 - k) * (1 - k);
                x += this.velocity[i * 3] * rest;
                y += this.velocity[i * 3 + 1] * rest;
                z += this.velocity[i * 3 + 2] * rest;
                scale = k;
                bright *= 0.6 + (1 - k) * 1.2;
            } else if (t > 0) {
                const k = Math.min(t / DISSOLVE_LIFE, 1);
                const drag = 1 - Math.exp(-3 * t); // кубики быстро теряют скорость и зависают
                x += (this.velocity[i * 3] * drag) / 3;
                y += (this.velocity[i * 3 + 1] * drag) / 3 + 0.12 * t * t;
                z += (this.velocity[i * 3 + 2] * drag) / 3;
                scale = (1 - k) * (1 + 0.6 * Math.max(0, 1 - t / 0.1));
                const flash = Math.max(0, 1 - t / 0.12); // вспышка на фронте распада
                r += (1 - r) * flash; g += (1 - g) * flash; b += (1 - b) * flash;
                bright = (bright + flash * 1.2) * (1 - k * k);
            }
            const m = i * 16;
            matrices[m] = scale; matrices[m + 5] = scale; matrices[m + 10] = scale;
            matrices[m + 12] = x; matrices[m + 13] = y; matrices[m + 14] = z;
            colors[i * 3] = r * bright; colors[i * 3 + 1] = g * bright; colors[i * 3 + 2] = b * bright;
        }
        this.fxMesh.instanceMatrix.needsUpdate = true;
        this.fxMesh.instanceColor.needsUpdate = true;
        if (this.fxTime >= this.fxDuration) {
            this.fxMesh.visible = false;
            if (assemble) this.setPartsVisible(true);
            this.fx = null;
            return true;
        }
        return false;
    }

    hide() {
        this.fx = null;
        this.fxMesh.visible = false;
        this.setPartsVisible(false);
    }
}

const DISSOLVE_LIFE = 0.75; // сколько секунд живёт отлетевший кубик
