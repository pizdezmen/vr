/**
 * Тренировочное измерение: тёмная пустота, светящаяся сетка пола,
 * далёкие столбы света и медленно плывущие «пиксели» в воздухе.
 */
import {
    AdditiveBlending,
    BoxGeometry,
    BufferAttribute,
    BufferGeometry,
    Color,
    DoubleSide,
    Group,
    InstancedMesh,
    Matrix4,
    MeshBasicMaterial,
    Mesh,
    PlaneGeometry,
    Points,
    PointsMaterial,
    ShaderMaterial,
} from '@iwsdk/core';

export function buildArena() {
    const group = new Group();

    // Пол: бесконечная на вид сетка, гаснущая вдали.
    const floor = new Mesh(
        new PlaneGeometry(120, 120),
        new ShaderMaterial({
            transparent: true,
            depthWrite: false,
            uniforms: { uTime: { value: 0 } },
            vertexShader: /* glsl */ `
                varying vec3 vPos;
                void main() {
                    vec4 world = modelMatrix * vec4(position, 1.0);
                    vPos = world.xyz;
                    gl_Position = projectionMatrix * viewMatrix * world;
                }
            `,
            fragmentShader: /* glsl */ `
                varying vec3 vPos;
                uniform float uTime;
                float gridLine(vec2 p, float width) {
                    vec2 g = abs(fract(p - 0.5) - 0.5) / fwidth(p);
                    return 1.0 - min(min(g.x, g.y) / width, 1.0);
                }
                void main() {
                    float dist = length(vPos.xz);
                    float fine = gridLine(vPos.xz * 2.0, 1.2) * smoothstep(9.0, 2.0, dist);
                    float coarse = gridLine(vPos.xz * 0.5, 1.6);
                    float fade = smoothstep(45.0, 3.0, dist);
                    float pulse = 0.5 + 0.5 * sin(dist * 1.2 - uTime * 1.5);
                    vec3 color = vec3(0.1, 0.45, 1.0) * (fine * 0.45 + coarse * (0.6 + 0.4 * pulse));
                    vec3 base = vec3(0.004, 0.012, 0.04);
                    gl_FragColor = vec4(base + color * fade, 1.0);
                }
            `,
        }),
    );
    floor.rotation.x = -Math.PI / 2;
    floor.renderOrder = -2;
    group.add(floor);

    // Столбы света по кругу.
    const pillarCount = 18;
    const pillars = new InstancedMesh(
        new BoxGeometry(0.5, 1, 0.5),
        new MeshBasicMaterial({
            color: 0x1a4fd0,
            transparent: true,
            opacity: 0.55,
            blending: AdditiveBlending,
            depthWrite: false,
        }),
        pillarCount,
    );
    const matrix = new Matrix4();
    const color = new Color();
    for (let i = 0; i < pillarCount; i++) {
        const angle = (i / pillarCount) * Math.PI * 2 + 0.17;
        const radius = 16 + ((i * 7) % 5) * 3;
        const height = 6 + ((i * 13) % 7) * 3;
        const width = 0.6 + ((i * 5) % 3) * 0.5;
        matrix.makeScale(width, height, width);
        matrix.setPosition(Math.cos(angle) * radius, height / 2, Math.sin(angle) * radius);
        pillars.setMatrixAt(i, matrix);
        pillars.setColorAt(i, color.setHSL(0.6 + ((i * 3) % 4) * 0.015, 0.9, 0.35 + ((i * 11) % 5) * 0.05));
    }
    pillars.frustumCulled = false;
    group.add(pillars);

    // Плывущие пиксели.
    const dustCount = 350;
    const positions = new Float32Array(dustCount * 3);
    for (let i = 0; i < dustCount; i++) {
        const angle = Math.random() * Math.PI * 2;
        const radius = 2.5 + Math.random() * 14;
        positions[i * 3] = Math.cos(angle) * radius;
        positions[i * 3 + 1] = Math.random() * 7;
        positions[i * 3 + 2] = Math.sin(angle) * radius;
    }
    const dustGeometry = new BufferGeometry();
    dustGeometry.setAttribute('position', new BufferAttribute(positions, 3));
    const dust = new Points(
        dustGeometry,
        new PointsMaterial({
            color: 0x58a6ff,
            size: 0.05,
            transparent: true,
            opacity: 0.7,
            blending: AdditiveBlending,
            depthWrite: false,
            side: DoubleSide,
        }),
    );
    dust.frustumCulled = false;
    group.add(dust);

    return {
        group,
        update(time) {
            floor.material.uniforms.uTime.value = time;
            dust.rotation.y = time * 0.01;
            dust.position.y = Math.sin(time * 0.2) * 0.2;
        },
    };
}
